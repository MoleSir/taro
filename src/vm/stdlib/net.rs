use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

use crate::{ToNativeData, NativeFunction, NativeData, ObjectHandle, ObjectInstanceData, ShrString};
use crate::vm::{ExecuteError, ExecuteResult, VirtualMachine};

// =============================================================================
//  Native data types
// =============================================================================

struct SocketData {
    stream: Option<TcpStream>,
    peer_addr: String,
}

impl ToNativeData for SocketData {}

struct ServerData {
    listener: Option<TcpListener>,
    bind_addr: String,
}

impl ToNativeData for ServerData {}

// =============================================================================
//  Module creation
// =============================================================================

impl VirtualMachine {
    /// Create the `net` std module.
    ///
    /// # Exports
    ///
    /// ## Socket class
    /// `Socket()` — TCP client socket.
    ///
    /// | method              | description                       |
    /// |---------------------|-----------------------------------|
    /// | `connect(h,p)`      | connect to host:port              |
    /// | `send(data)`        | send string data                  |
    /// | `recv(bufsize)`     | receive up to `bufsize` bytes     |
    /// | `close()`           | close the socket                  |
    /// | `settimeout(secs)`  | set read timeout in seconds       |
    ///
    /// ## Server class
    /// `Server()` — TCP listener.
    ///
    /// | method              | description                       |
    /// |---------------------|-----------------------------------|
    /// | `bind(port)`        | bind to 0.0.0.0:port              |
    /// | `bind(h, p)`        | bind to host:port                 |
    /// | `accept()`          | accept a connection → Socket      |
    /// | `close()`           | close the listener                |
    pub(super) fn create_net_module(&mut self) -> ExecuteResult<ObjectHandle> {
        // ---- Socket class ----
        let socket_class = self.obj_heap.alloc_class("Socket");
        self.register_native_method(socket_class, "connect",    NativeFunction::var(VirtualMachine::net_socket_connect));
        self.register_native_method(socket_class, "send",       NativeFunction::a2(VirtualMachine::net_socket_send));
        self.register_native_method(socket_class, "recv",       NativeFunction::a2(VirtualMachine::net_socket_recv));
        self.register_native_method(socket_class, "close",      NativeFunction::a1(VirtualMachine::net_socket_close));
        self.register_native_method(socket_class, "settimeout", NativeFunction::a2(VirtualMachine::net_socket_settimeout));
        self.register_native_method(socket_class, "__str__",    NativeFunction::a1(VirtualMachine::net_socket_str));

        // Store socket_class on the heap so Server.accept() can create Socket
        // instances without needing a closure capture.
        self.obj_heap.socket_class = socket_class;

        // ---- Server class ----
        let server_class = self.obj_heap.alloc_class("Server");
        self.register_native_method(server_class, "bind",   NativeFunction::var(VirtualMachine::net_server_bind));
        self.register_native_method(server_class, "accept", NativeFunction::a1(VirtualMachine::net_server_accept));
        self.register_native_method(server_class, "close",  NativeFunction::a1(VirtualMachine::net_server_close));
        self.register_native_method(server_class, "__str__", NativeFunction::a1(VirtualMachine::net_server_str));

        // ---- assemble module ----
        let mut exports: HashMap<ShrString, ObjectHandle> = HashMap::new();
        exports.insert(ShrString::new_str("Socket"), socket_class);
        exports.insert(ShrString::new_str("Server"), server_class);

        let module = self.obj_heap.alloc_fields_instance(self.obj_heap.module_class);
        if let Some(inst) = self.obj_heap.get_instance_mut(module) {
            if let ObjectInstanceData::Fields(fields) = &mut inst.data {
                *fields = exports;
            }
        }

        Ok(module)
    }

    // =====================================================================
    //  Socket — connect
    // =====================================================================

    /// `socket.connect(host, port)` or `socket.connect("host:port")`
    fn net_socket_connect(&mut self, args: &[ObjectHandle]) -> ExecuteResult<ObjectHandle> {
        // args[0] = receiver (self)
        let explicit = args.len().saturating_sub(1);
        if explicit < 1 || explicit > 2 {
            return Err(ExecuteError::ArgumentCountMismatch { expected: 1, got: explicit });
        }
        let self_handle = args[0];

        let addr = if let Some(&port_handle) = args.get(2) {
            // Two-arg form: connect("host", port)
            let host = self.get_string_instance(args[1])?.as_str().to_string();
            let port = *self.get_integer_instance(port_handle)?;
            format!("{}:{}", host, port)
        } else {
            // One-arg form: connect("host:port")
            self.get_string_instance(args[1])?.as_str().to_string()
        };

        let stream = TcpStream::connect(&addr)
            .map_err(|e| ExecuteError::NetError(format!("cannot connect to '{}': {}", addr, e)))?;

        let inst = self.obj_heap.get_instance_mut(self_handle)
            .ok_or_else(|| ExecuteError::NetError("not a Socket instance".into()))?;
        inst.data = ObjectInstanceData::Native(
            NativeData::new(SocketData { stream: Some(stream), peer_addr: addr }),
        );

        Ok(self_handle)
    }

    // =====================================================================
    //  Socket — send
    // =====================================================================

    /// `socket.send(data)` — send a string.
    fn net_socket_send(&mut self, receiver: ObjectHandle, data: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let text = self.get_string_instance(data)?.clone();
        let stream = self.get_native_mut::<SocketData>(receiver)?.stream.as_mut()
            .ok_or_else(|| ExecuteError::NetError("socket is closed".into()))?;
        stream.write_all(text.as_bytes())
            .map_err(|e| ExecuteError::NetError(format!("send error: {}", e)))?;
        Ok(ObjectHandle::NIL)
    }

    // =====================================================================
    //  Socket — recv
    // =====================================================================

    /// `socket.recv(bufsize)` — receive up to `bufsize` bytes, return as string.
    fn net_socket_recv(&mut self, receiver: ObjectHandle, bufsize: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let n = *self.get_integer_instance(bufsize)?;
        if n <= 0 || n > 65536 {
            return Err(ExecuteError::NetError(format!(
                "recv: bufsize must be 1..65536, got {}", n
            )));
        }
        let stream = self.get_native_mut::<SocketData>(receiver)?.stream.as_mut()
            .ok_or_else(|| ExecuteError::NetError("socket is closed".into()))?;
        let mut buf = vec![0u8; n as usize];
        let read = stream.read(&mut buf)
            .map_err(|e| ExecuteError::NetError(format!("recv error: {}", e)))?;
        buf.truncate(read);
        let s = String::from_utf8_lossy(&buf).to_string();
        Ok(self.obj_heap.alloc_string_instance(ShrString::new_string(&s)))
    }

    // =====================================================================
    //  Socket — close
    // =====================================================================

    /// `socket.close()` — close the socket.
    fn net_socket_close(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        self.get_native_mut::<SocketData>(receiver)?.stream = None;
        Ok(ObjectHandle::NIL)
    }

    // =====================================================================
    //  Socket — settimeout
    // =====================================================================

    /// `socket.settimeout(seconds)` — set the read timeout.
    fn net_socket_settimeout(&mut self, receiver: ObjectHandle, seconds: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let secs = if let Ok(v) = self.get_float_instance(seconds) {
            *v
        } else if let Ok(v) = self.get_integer_instance(seconds) {
            *v as f64
        } else {
            return Err(ExecuteError::UnexpectedType("number", self.value_type_name(seconds)));
        };
        let dur = Duration::from_secs_f64(secs);
        let data = self.get_native_mut::<SocketData>(receiver)?;
        if let Some(ref stream) = data.stream {
            stream.set_read_timeout(Some(dur))
                .map_err(|e| ExecuteError::NetError(format!("settimeout: {}", e)))?;
        }
        Ok(ObjectHandle::NIL)
    }

    // =====================================================================
    //  Socket — __str__
    // =====================================================================

    /// `socket.__str__()` → `<Socket peer='host:port' status=open|closed>`
    fn net_socket_str(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let (is_open, addr) = if let Some(d) = self.obj_heap.get_native::<SocketData>(receiver) {
            (d.stream.is_some(), d.peer_addr.clone())
        } else {
            (false, "?".into())
        };
        let status = if is_open { "open" } else { "closed" };
        Ok(self.obj_heap.alloc_string_instance(ShrString::new_string(&format!(
            "<Socket peer='{}' status={}>", addr, status
        ))))
    }

    // =====================================================================
    //  Server — bind
    // =====================================================================

    /// `server.bind(port)` or `server.bind(host, port)` or `server.bind("host:port")`
    fn net_server_bind(&mut self, args: &[ObjectHandle]) -> ExecuteResult<ObjectHandle> {
        let explicit = args.len().saturating_sub(1);
        if explicit < 1 || explicit > 2 {
            return Err(ExecuteError::ArgumentCountMismatch { expected: 1, got: explicit });
        }
        let self_handle = args[0];

        let addr = if explicit == 2 {
            // Two-arg form: bind(host, port)
            let host = self.get_string_instance(args[1])?.as_str().to_string();
            let port = *self.get_integer_instance(args[2])?;
            format!("{}:{}", host, port)
        } else if let Ok(port) = self.get_integer_instance(args[1]) {
            // One-arg: bind(port) → bind 0.0.0.0:port
            format!("0.0.0.0:{}", port)
        } else {
            // One-arg: bind("host:port")
            self.get_string_instance(args[1])?.as_str().to_string()
        };

        let listener = TcpListener::bind(&addr)
            .map_err(|e| ExecuteError::NetError(format!("cannot bind '{}': {}", addr, e)))?;

        let inst = self.obj_heap.get_instance_mut(self_handle)
            .ok_or_else(|| ExecuteError::NetError("not a Server instance".into()))?;
        inst.data = ObjectInstanceData::Native(
            NativeData::new(ServerData { listener: Some(listener), bind_addr: addr }),
        );

        Ok(self_handle)
    }

    // =====================================================================
    //  Server — accept
    // =====================================================================

    /// `server.accept()` — accept a connection, return a Socket instance.
    fn net_server_accept(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let listener = self.get_native_mut::<ServerData>(receiver)?.listener.as_mut()
            .ok_or_else(|| ExecuteError::NetError("server is closed".into()))?;
        let (stream, peer_addr) = listener.accept()
            .map_err(|e| ExecuteError::NetError(format!("accept error: {}", e)))?;
        let peer_str = peer_addr.to_string();

        // Create a Socket instance using the cached socket_class.
        let socket_class = self.obj_heap.socket_class;
        Ok(self.obj_heap.alloc_instance(socket_class, ObjectInstanceData::Native(
            NativeData::new(SocketData { stream: Some(stream), peer_addr: peer_str }),
        )))
    }

    // =====================================================================
    //  Server — close
    // =====================================================================

    /// `server.close()` — close the listener.
    fn net_server_close(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        self.get_native_mut::<ServerData>(receiver)?.listener = None;
        Ok(ObjectHandle::NIL)
    }

    // =====================================================================
    //  Server — __str__
    // =====================================================================

    /// `server.__str__()` → `<Server addr='host:port' status=open|closed>`
    fn net_server_str(&mut self, receiver: ObjectHandle) -> ExecuteResult<ObjectHandle> {
        let (is_open, addr) = if let Some(d) = self.obj_heap.get_native::<ServerData>(receiver) {
            (d.listener.is_some(), d.bind_addr.clone())
        } else {
            (false, "?".into())
        };
        let status = if is_open { "open" } else { "closed" };
        Ok(self.obj_heap.alloc_string_instance(ShrString::new_string(&format!(
            "<Server addr='{}' status={}>", addr, status
        ))))
    }
}

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;
use crate::{ToNativeData, NativeFunction, NativeData, ObjectHandle, ObjectInstanceData, ShrString};
use crate::vm::{RuntimeResult, RuntimeErrorKind, VirtualMachine};

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
    pub(crate) fn create_net_module(&mut self) -> RuntimeResult<ObjectHandle> {
        // ---- Socket class ----
        let socket_class = self.obj_heap.alloc_class("Socket");
        self.register_native_method(socket_class, "connect",    NativeFunction::var(StdSocketData::net_socket_connect));
        self.register_native_method(socket_class, "send",       NativeFunction::a2(StdSocketData::net_socket_send));
        self.register_native_method(socket_class, "recv",       NativeFunction::a2(StdSocketData::net_socket_recv));
        self.register_native_method(socket_class, "close",      NativeFunction::a1(StdSocketData::net_socket_close));
        self.register_native_method(socket_class, "settimeout", NativeFunction::a2(StdSocketData::net_socket_settimeout));
        self.register_native_method(socket_class, "__str__",    NativeFunction::a1(StdSocketData::net_socket_str));

        // Store socket_class on the heap so Server.accept() can create Socket
        // instances without needing a closure capture.
        self.obj_heap.socket_class = socket_class;

        // ---- Server class ----
        let server_class = self.obj_heap.alloc_class("Server");
        self.register_native_method(server_class, "bind",   NativeFunction::var(StdSocketData::net_server_bind));
        self.register_native_method(server_class, "accept", NativeFunction::a1(StdSocketData::net_server_accept));
        self.register_native_method(server_class, "close",  NativeFunction::a1(StdSocketData::net_server_close));
        self.register_native_method(server_class, "__str__", NativeFunction::a1(StdSocketData::net_server_str));

        // ---- assemble module ----
        let mut exports: HashMap<ShrString, ObjectHandle> = HashMap::new();
        exports.insert(ShrString::new_str("Socket"), socket_class);
        exports.insert(ShrString::new_str("Server"), server_class);

        let module = self.obj_heap.alloc_fields_instance(self.obj_heap.module_class, exports);
        Ok(module)
    }
}

struct StdSocketData {
    stream: Option<TcpStream>,
    peer_addr: String,
}

impl ToNativeData for StdSocketData {}

struct ServerData {
    listener: Option<TcpListener>,
    bind_addr: String,
}

impl ToNativeData for ServerData {}

impl StdSocketData {
    /// `socket.connect(host, port)` or `socket.connect("host:port")`
    fn net_socket_connect(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        // args[0] = receiver (self)
        let explicit = args.len().saturating_sub(1);
        if explicit < 1 || explicit > 2 {
            return Err(RuntimeErrorKind::ArgumentCountMismatch { expected: 1, got: explicit });
        }
        let self_handle = args[0];

        let addr = if let Some(&port_handle) = args.get(2) {
            // Two-arg form: connect("host", port)
            let host = vm.get_string_instance(args[1])?.as_str().to_string();
            let port = *vm.get_integer_instance(port_handle)?;
            format!("{}:{}", host, port)
        } else {
            // One-arg form: connect("host:port")
            vm.get_string_instance(args[1])?.as_str().to_string()
        };

        let stream = TcpStream::connect(&addr)
            .map_err(|e| RuntimeErrorKind::NetError(format!("cannot connect to '{}': {}", addr, e)))?;

        let inst = vm.obj_heap.get_instance_mut(self_handle)
            .ok_or_else(|| RuntimeErrorKind::NetError("not a Socket instance".into()))?;
        inst.data = ObjectInstanceData::Native(
            NativeData::new(StdSocketData { stream: Some(stream), peer_addr: addr }),
        );

        Ok(self_handle)
    }

    /// `socket.send(data)` — send a string.
    fn net_socket_send(vm: &mut VirtualMachine, receiver: ObjectHandle, data: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let text = vm.get_string_instance(data)?.clone();
        let stream = vm.get_native_mut::<StdSocketData>(receiver)?.stream.as_mut()
            .ok_or_else(|| RuntimeErrorKind::NetError("socket is closed".into()))?;
        stream.write_all(text.as_bytes())
            .map_err(|e| RuntimeErrorKind::NetError(format!("send error: {}", e)))?;
        Ok(ObjectHandle::NIL)
    }

    /// `socket.recv(bufsize)` — receive up to `bufsize` bytes, return as string.
    fn net_socket_recv(vm: &mut VirtualMachine, receiver: ObjectHandle, bufsize: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let n = *vm.get_integer_instance(bufsize)?;
        if n <= 0 || n > 65536 {
            return Err(RuntimeErrorKind::NetError(format!(
                "recv: bufsize must be 1..65536, got {}", n
            )));
        }
        let stream = vm.get_native_mut::<StdSocketData>(receiver)?.stream.as_mut()
            .ok_or_else(|| RuntimeErrorKind::NetError("socket is closed".into()))?;
        let mut buf = vec![0u8; n as usize];
        let read = stream.read(&mut buf)
            .map_err(|e| RuntimeErrorKind::NetError(format!("recv error: {}", e)))?;
        buf.truncate(read);
        let s = String::from_utf8_lossy(&buf).to_string();
        Ok(vm.obj_heap.alloc_string_instance(ShrString::new_string(&s)))
    }

    /// `socket.close()` — close the socket.
    fn net_socket_close(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        vm.get_native_mut::<StdSocketData>(receiver)?.stream = None;
        Ok(ObjectHandle::NIL)
    }

    /// `socket.settimeout(seconds)` — set the read timeout.
    fn net_socket_settimeout(vm: &mut VirtualMachine, receiver: ObjectHandle, seconds: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let secs = if let Ok(v) = vm.get_float_instance(seconds) {
            *v
        } else if let Ok(v) = vm.get_integer_instance(seconds) {
            *v as f64
        } else {
            return Err(RuntimeErrorKind::UnexpectedType("number", vm.value_type_name(seconds)));
        };
        let dur = Duration::from_secs_f64(secs);
        let data = vm.get_native_mut::<StdSocketData>(receiver)?;
        if let Some(ref stream) = data.stream {
            stream.set_read_timeout(Some(dur))
                .map_err(|e| RuntimeErrorKind::NetError(format!("settimeout: {}", e)))?;
        }
        Ok(ObjectHandle::NIL)
    }

    /// `socket.__str__()` → `<Socket peer='host:port' status=open|closed>`
    fn net_socket_str(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let (is_open, addr) = if let Some(d) = vm.obj_heap.get_native::<StdSocketData>(receiver) {
            (d.stream.is_some(), d.peer_addr.clone())
        } else {
            (false, "?".into())
        };
        let status = if is_open { "open" } else { "closed" };
        Ok(vm.obj_heap.alloc_string_instance(ShrString::new_string(&format!(
            "<Socket peer='{}' status={}>", addr, status
        ))))
    }

    /// `server.bind(port)` or `server.bind(host, port)` or `server.bind("host:port")`
    fn net_server_bind(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        let explicit = args.len().saturating_sub(1);
        if explicit < 1 || explicit > 2 {
            return Err(RuntimeErrorKind::ArgumentCountMismatch { expected: 1, got: explicit });
        }
        let self_handle = args[0];

        let addr = if explicit == 2 {
            // Two-arg form: bind(host, port)
            let host = vm.get_string_instance(args[1])?.as_str().to_string();
            let port = *vm.get_integer_instance(args[2])?;
            format!("{}:{}", host, port)
        } else if let Ok(port) = vm.get_integer_instance(args[1]) {
            // One-arg: bind(port) → bind 0.0.0.0:port
            format!("0.0.0.0:{}", port)
        } else {
            // One-arg: bind("host:port")
            vm.get_string_instance(args[1])?.as_str().to_string()
        };

        let listener = TcpListener::bind(&addr)
            .map_err(|e| RuntimeErrorKind::NetError(format!("cannot bind '{}': {}", addr, e)))?;

        let inst = vm.obj_heap.get_instance_mut(self_handle)
            .ok_or_else(|| RuntimeErrorKind::NetError("not a Server instance".into()))?;
        inst.data = ObjectInstanceData::Native(
            NativeData::new(ServerData { listener: Some(listener), bind_addr: addr }),
        );

        Ok(self_handle)
    }

    /// `server.accept()` — accept a connection, return a Socket instance.
    fn net_server_accept(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let listener = vm.get_native_mut::<ServerData>(receiver)?.listener.as_mut()
            .ok_or_else(|| RuntimeErrorKind::NetError("server is closed".into()))?;
        let (stream, peer_addr) = listener.accept()
            .map_err(|e| RuntimeErrorKind::NetError(format!("accept error: {}", e)))?;
        let peer_str = peer_addr.to_string();

        // Create a Socket instance using the cached socket_class.
        let socket_class = vm.obj_heap.socket_class;
        Ok(vm.obj_heap.alloc_instance(socket_class, ObjectInstanceData::Native(
            NativeData::new(StdSocketData { stream: Some(stream), peer_addr: peer_str }),
        )))
    }

    /// `server.close()` — close the listener.
    fn net_server_close(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        vm.get_native_mut::<ServerData>(receiver)?.listener = None;
        Ok(ObjectHandle::NIL)
    }

    /// `server.__str__()` → `<Server addr='host:port' status=open|closed>`
    fn net_server_str(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let (is_open, addr) = if let Some(d) = vm.obj_heap.get_native::<ServerData>(receiver) {
            (d.listener.is_some(), d.bind_addr.clone())
        } else {
            (false, "?".into())
        };
        let status = if is_open { "open" } else { "closed" };
        Ok(vm.obj_heap.alloc_string_instance(ShrString::new_string(&format!(
            "<Server addr='{}' status={}>", addr, status
        ))))
    }
}

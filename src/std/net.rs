use super::ModuleBuilder;
use crate::vm::{RuntimeErrorKind, RuntimeResult, VirtualMachine};
use crate::{NativeFunction, ObjectHandle, ShrString, impl_object_instance_data};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

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
        let mut m = ModuleBuilder::new(&mut self.obj_heap, "net");

        m.define_class("Socket", |class| {
            class.method("__new__", NativeFunction::var(Socket::__new__));
            class.method("connect", NativeFunction::var(Socket::connect));
            class.method("send", NativeFunction::a2(Socket::send));
            class.method("recv", NativeFunction::a2(Socket::recv));
            class.method("close", NativeFunction::a1(Socket::close));
            class.method("settimeout", NativeFunction::a2(Socket::settimeout));
            class.method("__str__", NativeFunction::a1(Socket::__str__));
        });

        m.define_class("Server", |class| {
            class.method("__new__", NativeFunction::var(Server::__new__));
            class.method("bind", NativeFunction::var(Server::bind));
            class.method("accept", NativeFunction::a1(Server::accept));
            class.method("close", NativeFunction::a1(Server::close));
            class.method("__str__", NativeFunction::a1(Server::__str__));
        });

        Ok(m.build())
    }
}

struct Socket {
    stream: Option<TcpStream>,
    peer_addr: String,
}

impl_object_instance_data!(Socket, "Socket");

struct Server {
    listener: Option<TcpListener>,
    bind_addr: String,
}

impl_object_instance_data!(Server, "Server");

impl Socket {
    fn __new__(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        let class = args[0];
        Ok(vm.obj_heap.alloc_instance_dyn(class, Box::new(Socket { stream: None, peer_addr: String::new() })))
    }

    /// `socket.connect(host, port)` or `socket.connect("host:port")`
    fn connect(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        // args[0] = receiver (self)
        let explicit = args.len().saturating_sub(1);
        if explicit < 1 || explicit > 2 {
            return Err(RuntimeErrorKind::ArgumentCountMismatch { expected: 1, got: explicit });
        }
        let self_handle = args[0];

        let addr = if let Some(&port_handle) = args.get(2) {
            // Two-arg form: connect("host", port)
            let host = vm.obj_heap.expect_string(args[1])?.as_str().to_string();
            let port = *vm.obj_heap.expect_integer(port_handle)?;
            format!("{}:{}", host, port)
        } else {
            // One-arg form: connect("host:port")
            vm.obj_heap.expect_string(args[1])?.as_str().to_string()
        };

        let stream = TcpStream::connect(&addr).map_err(|e| RuntimeErrorKind::NetError(format!("cannot connect to '{}': {}", addr, e)))?;

        let inst = vm.obj_heap.get_instance_mut(self_handle).ok_or_else(|| RuntimeErrorKind::NetError("not a Socket instance".into()))?;
        inst.data = Box::new(Socket { stream: Some(stream), peer_addr: addr });

        Ok(self_handle)
    }

    /// `socket.send(data)` — send a string.
    fn send(vm: &mut VirtualMachine, receiver: ObjectHandle, data: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let text = vm.obj_heap.expect_string(data)?.clone();
        let found = vm.obj_heap.type_of(receiver);
        let stream = vm
            .obj_heap
            .get_instance_data_mut::<Socket>(receiver)
            .ok_or_else(|| RuntimeErrorKind::TypeMismatch { expected: "native", found })?
            .stream
            .as_mut()
            .ok_or_else(|| RuntimeErrorKind::NetError("socket is closed".into()))?;
        stream.write_all(text.as_bytes()).map_err(|e| RuntimeErrorKind::NetError(format!("send error: {}", e)))?;
        Ok(ObjectHandle::NIL)
    }

    /// `socket.recv(bufsize)` — receive up to `bufsize` bytes, return as string.
    fn recv(vm: &mut VirtualMachine, receiver: ObjectHandle, bufsize: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let n = *vm.obj_heap.expect_integer(bufsize)?;
        if n <= 0 || n > 65536 {
            return Err(RuntimeErrorKind::NetError(format!("recv: bufsize must be 1..65536, got {}", n)));
        }
        let found = vm.obj_heap.type_of(receiver);
        let stream = vm
            .obj_heap
            .get_instance_data_mut::<Socket>(receiver)
            .ok_or_else(|| RuntimeErrorKind::TypeMismatch { expected: "native", found })?
            .stream
            .as_mut()
            .ok_or_else(|| RuntimeErrorKind::NetError("socket is closed".into()))?;
        let mut buf = vec![0u8; n as usize];
        let read = stream.read(&mut buf).map_err(|e| RuntimeErrorKind::NetError(format!("recv error: {}", e)))?;
        buf.truncate(read);
        let s = String::from_utf8_lossy(&buf).to_string();
        Ok(vm.obj_heap.alloc_string_instance(ShrString::new_string(&s)))
    }

    /// `socket.close()` — close the socket.
    fn close(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let found = vm.obj_heap.type_of(receiver);
        vm.obj_heap
            .get_instance_data_mut::<Socket>(receiver)
            .ok_or_else(|| RuntimeErrorKind::TypeMismatch { expected: "native", found })?
            .stream = None;
        Ok(ObjectHandle::NIL)
    }

    /// `socket.settimeout(seconds)` — set the read timeout.
    fn settimeout(vm: &mut VirtualMachine, receiver: ObjectHandle, seconds: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let secs = if let Some(v) = vm.obj_heap.get_float_instance(seconds) {
            *v
        } else if let Some(v) = vm.obj_heap.get_integer_instance(seconds) {
            *v as f64
        } else {
            return Err(RuntimeErrorKind::UnexpectedType("number", vm.obj_heap.type_of(seconds)));
        };
        let dur = Duration::from_secs_f64(secs);
        let found = vm.obj_heap.type_of(receiver);
        let data = vm
            .obj_heap
            .get_instance_data_mut::<Socket>(receiver)
            .ok_or_else(|| RuntimeErrorKind::TypeMismatch { expected: "native", found })?;
        if let Some(ref stream) = data.stream {
            stream.set_read_timeout(Some(dur)).map_err(|e| RuntimeErrorKind::NetError(format!("settimeout: {}", e)))?;
        }
        Ok(ObjectHandle::NIL)
    }

    /// `socket.__str__()` → `<Socket peer='host:port' status=open|closed>`
    fn __str__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let (is_open, addr) = if let Some(d) = vm.obj_heap.get_instance_data::<Socket>(receiver) {
            (d.stream.is_some(), d.peer_addr.clone())
        } else {
            (false, "?".into())
        };
        let status = if is_open { "open" } else { "closed" };
        Ok(vm.obj_heap.alloc_string_instance(ShrString::new_string(&format!("<Socket peer='{}' status={}>", addr, status))))
    }
}

impl Server {
    fn __new__(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        let class = args[0];
        Ok(vm.obj_heap.alloc_instance_dyn(class, Box::new(Server { listener: None, bind_addr: "".into() })))
    }

    /// `server.bind(port)` or `server.bind(host, port)` or `server.bind("host:port")`
    fn bind(vm: &mut VirtualMachine, args: &[ObjectHandle]) -> RuntimeResult<ObjectHandle> {
        let explicit = args.len().saturating_sub(1);
        if explicit < 1 || explicit > 2 {
            return Err(RuntimeErrorKind::ArgumentCountMismatch { expected: 1, got: explicit });
        }
        let self_handle = args[0];

        let addr = if explicit == 2 {
            // Two-arg form: bind(host, port)
            let host = vm.obj_heap.expect_string(args[1])?.as_str().to_string();
            let port = *vm.obj_heap.expect_integer(args[2])?;
            format!("{}:{}", host, port)
        } else if let Some(port) = vm.obj_heap.get_integer_instance(args[1]) {
            // One-arg: bind(port) → bind 0.0.0.0:port
            format!("0.0.0.0:{}", port)
        } else {
            // One-arg: bind("host:port")
            vm.obj_heap.expect_string(args[1])?.as_str().to_string()
        };

        let listener = TcpListener::bind(&addr).map_err(|e| RuntimeErrorKind::NetError(format!("cannot bind '{}': {}", addr, e)))?;

        let inst = vm
            .obj_heap
            .get_instance_mut(self_handle)
            .ok_or_else(|| RuntimeErrorKind::NetError("not a Server instance".into()))?
            .get_data_mut::<Server>()
            .expect("must ");
        inst.listener = Some(listener);
        inst.bind_addr = addr;

        Ok(self_handle)
    }

    /// `server.accept()` — accept a connection, return a Socket instance.
    fn accept(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let found = vm.obj_heap.type_of(receiver);
        let listener = vm
            .obj_heap
            .get_instance_data_mut::<Server>(receiver)
            .ok_or_else(|| RuntimeErrorKind::TypeMismatch { expected: "native", found })?
            .listener
            .as_mut()
            .ok_or_else(|| RuntimeErrorKind::NetError("server is closed".into()))?;
        let (stream, peer_addr) = listener.accept().map_err(|e| RuntimeErrorKind::NetError(format!("accept error: {}", e)))?;
        let peer_str = peer_addr.to_string();

        // Look up the Socket class via the module back-reference on the Server
        // class, so we don't need a dedicated socket_class field on ObjectHeap.
        let socket_class = vm
            .lookup_module_export(receiver, "Socket")
            .ok_or_else(|| RuntimeErrorKind::NetError("Socket class not found in net module".into()))?;
        Ok(vm.obj_heap.alloc_instance(socket_class, Socket { stream: Some(stream), peer_addr: peer_str }))
    }

    /// `server.close()` — close the listener.
    fn close(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let found = vm.obj_heap.type_of(receiver);
        vm.obj_heap
            .get_instance_data_mut::<Server>(receiver)
            .ok_or_else(|| RuntimeErrorKind::TypeMismatch { expected: "native", found })?
            .listener = None;
        Ok(ObjectHandle::NIL)
    }

    /// `server.__str__()` → `<Server addr='host:port' status=open|closed>`
    fn __str__(vm: &mut VirtualMachine, receiver: ObjectHandle) -> RuntimeResult<ObjectHandle> {
        let (is_open, addr) = if let Some(d) = vm.obj_heap.get_instance_data::<Server>(receiver) {
            (d.listener.is_some(), d.bind_addr.clone())
        } else {
            (false, "?".into())
        };
        let status = if is_open { "open" } else { "closed" };
        Ok(vm.obj_heap.alloc_string_instance(ShrString::new_string(&format!("<Server addr='{}' status={}>", addr, status))))
    }
}

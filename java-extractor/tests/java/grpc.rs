use java_extractor::extraction::extract_syntactic;

const EXPECTED_FLIGHT_SERVICE_URIS: [&str; 2] = [
    "grpc://FlightService/GetById",
    "grpc://FlightService/ReserveSeat",
];

#[test]
fn identifies_generated_blocking_stub_operations() {
    let code = r#"
        class BookingHandler {
            private final FlightServiceGrpc.FlightServiceBlockingStub flightStub;
            void handle() {
                flightStub.getById(request);
                flightStub.reserveSeat(reservation);
            }
        }
    "#;

    let record = extract_syntactic(code, "BookingHandler.java").unwrap();
    let uris: Vec<_> = record
        .raw_restcalls
        .iter()
        .map(|call| call.target_uri.as_str())
        .collect();
    assert_eq!(uris, EXPECTED_FLIGHT_SERVICE_URIS.to_vec());
}

#[test]
fn identifies_grpc_service_methods_as_operations() {
    let code = r#"
        public class FlightServiceGrpcImpl extends FlightServiceGrpc.FlightServiceImplBase {
            public void getById(GetByIdRequest request, StreamObserver<Response> observer) {}
            public void reserveSeat(ReserveSeatRequest request, StreamObserver<Response> observer) {}
        }
    "#;

    let record = extract_syntactic(code, "FlightServiceGrpcImpl.java").unwrap();
    let uris: Vec<_> = record
        .endpoints
        .iter()
        .map(|endpoint| endpoint.uri.as_str())
        .collect();
    assert_eq!(uris, EXPECTED_FLIGHT_SERVICE_URIS.to_vec());
}

#[test]
fn identifies_local_and_direct_stub_calls_and_manual_service_binding() {
    let client = r#"
        class Client {
            void calls(Channel channel) {
                GreeterBlockingStub stub = GreeterGrpc.newBlockingStub(channel);
                stub.sayHello(request);
                GreeterGrpc.newFutureStub(channel).sayHello(request);
            }
        }
    "#;
    let client_record = extract_syntactic(client, "Client.java").unwrap();
    let uris: Vec<_> = client_record
        .raw_restcalls
        .iter()
        .map(|call| call.target_uri.as_str())
        .collect();
    assert_eq!(
        uris,
        vec!["grpc://Greeter/SayHello", "grpc://Greeter/SayHello"]
    );

    let server = r#"
        class GreeterImpl implements BindableService {
            private void sayHello(Request request, StreamObserver<Response> observer) {}
            public ServerServiceDefinition bindService() {
                return ServerServiceDefinition.builder(GreeterGrpc.getServiceDescriptor().getName())
                    .addMethod(METHOD_SAY_HELLO, handler)
                    .build();
            }
        }
    "#;
    let server_record = extract_syntactic(server, "GreeterImpl.java").unwrap();
    let uris: Vec<_> = server_record
        .endpoints
        .iter()
        .map(|endpoint| endpoint.uri.as_str())
        .collect();
    assert_eq!(uris, vec!["grpc://Greeter/SayHello"]);
}

#[test]
fn identifies_grpc_spring_imported_stubs_and_implementations() {
    let client = r#"
        import example.SimpleGrpc.SimpleBlockingStub;
        class Client {
            @GrpcClient("server")
            private SimpleBlockingStub simpleStub;
            void call() { this.simpleStub.sayHello(request); }
        }
    "#;
    let client_record = extract_syntactic(client, "Client.java").unwrap();
    assert_eq!(
        client_record.raw_restcalls[0].target_uri,
        "grpc://Simple/SayHello"
    );

    let server = r#"
        import example.SimpleGrpc.SimpleImplBase;
        class Service extends SimpleImplBase {
            public StreamObserver<Request> stream(StreamObserver<Response> observer) { return null; }
        }
    "#;
    let server_record = extract_syntactic(server, "Service.java").unwrap();
    assert_eq!(server_record.endpoints[0].uri, "grpc://Simple/Stream");
}

#[test]
fn identifies_grpc_spring_streaming_implementation_methods() {
    let server = r#"
        import example.ExampleServiceGrpc.ExampleServiceImplBase;
        class Service extends ExampleServiceImplBase {
            public void unaryRpc(UnaryRequest request, StreamObserver<UnaryResponse> observer) {}
            public StreamObserver<ClientStreamingRequest> clientStreamingRpc(
                StreamObserver<ClientStreamingResponse> observer) { return null; }
            public void serverStreamingRpc(ServerStreamingRequest request,
                StreamObserver<ServerStreamingResponse> observer) {}
            public StreamObserver<BidiStreamingRequest> bidiStreamingRpc(
                StreamObserver<BidiStreamingResponse> observer) {
                return new StreamObserver<>() {
                    public void onNext(BidiStreamingRequest request) {}
                    public void onError(Throwable error) {}
                    public void onCompleted() {}
                };
            }
        }
    "#;
    let record = extract_syntactic(server, "Service.java").unwrap();
    let uris: Vec<_> = record
        .endpoints
        .iter()
        .map(|endpoint| endpoint.uri.as_str())
        .collect();
    assert_eq!(
        uris,
        vec![
            "grpc://ExampleService/UnaryRpc",
            "grpc://ExampleService/ClientStreamingRpc",
            "grpc://ExampleService/ServerStreamingRpc",
            "grpc://ExampleService/BidiStreamingRpc",
        ]
    );
}

#[test]
fn identifies_armeria_grpc_client_factories_and_excludes_service_helpers() {
    let client = r#"
        class Client {
            void call(String uri) {
                var blocking = GrpcClients.newClient(uri, HelloServiceBlockingStub.class);
                var async = GrpcClients.builder(uri).build(HelloServiceStub.class);
                blocking.sayHello(request);
                async.streamHello(request);
            }
        }
    "#;
    let client_record = extract_syntactic(client, "Client.java").unwrap();
    let uris: Vec<_> = client_record
        .raw_restcalls
        .iter()
        .map(|call| call.target_uri.as_str())
        .collect();
    assert_eq!(
        uris,
        vec!["grpc://HelloService/SayHello", "grpc://HelloService/StreamHello"]
    );

    let server = r#"
        class HelloService extends HelloServiceGrpc.HelloServiceImplBase {
            public void sayHello(Request request, StreamObserver<Response> observer) {}
            private static Response buildReply(Object value) { return null; }
        }
    "#;
    let server_record = extract_syntactic(server, "HelloService.java").unwrap();
    assert_eq!(server_record.endpoints.len(), 1);
    assert_eq!(server_record.endpoints[0].uri, "grpc://HelloService/SayHello");
}

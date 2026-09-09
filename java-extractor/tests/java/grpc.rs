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

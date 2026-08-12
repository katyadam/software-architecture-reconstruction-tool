use java_extractor::extraction::extract_syntactic;

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
    assert_eq!(
        uris,
        vec![
            "grpc://FlightService/GetById",
            "grpc://FlightService/ReserveSeat"
        ]
    );
}

#[test]
fn identifies_grpc_service_methods_as_operations() {
    let code = r#"
        @GrpcService
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
    assert_eq!(
        uris,
        vec![
            "grpc://FlightService/GetById",
            "grpc://FlightService/ReserveSeat"
        ]
    );
}

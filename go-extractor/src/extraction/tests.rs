use std::path::Path;

use super::{extract_syntactic, identify, should_extract_file};

#[test]
fn extracts_train_ticket_routes_and_exchange_calls() {
    let code = r#"
const basePath = "/api/v1/stationservice"

func NewRouter() {
    mux := http.NewServeMux()
    mux.HandleFunc("GET "+basePath+"/stations", handler)
}

func handler() {}

const routeServiceName = "ts-route-service"

func (c *RouteClient) RoutesBetween(start, end string) {
    path := "/api/v1/routeservice/routes/" + url.PathEscape(start) + "/" + url.PathEscape(end)
    _ = c.transport.exchange(ctx, routeServiceName, http.MethodGet, path, nil, &response)
}
"#;

    let record = extract_syntactic(code, "router.go").expect("Go extraction should succeed");
    assert_eq!(record.endpoints.len(), 1);
    assert_eq!(record.endpoints[0].uri, "/api/v1/stationservice/stations");
    assert_eq!(record.call_statements.len(), 5);
    assert!(
        record
            .assignments
            .values()
            .any(|assignment| assignment.variable_name == "path"
                && assignment.value == "/api/v1/routeservice/routes/{start}/{end}")
    );

    let mut typed = models::ir::project::TypedFileRecord::from(record);
    identify(&mut typed);
    assert_eq!(typed.raw_restcalls.len(), 1);
    assert_eq!(
        typed.raw_restcalls[0].target_uri,
        "http://ts-route-service/api/v1/routeservice/routes/{start}/{end}"
    );
}

#[test]
fn extracts_gorilla_and_direct_http_calls() {
    let code = r#"
func UpdatePaymentStatus() {}

func Router() {
    r.HandleFunc("/payment/{order_id}", UpdatePaymentStatus).Methods("POST")
}

func invoke(url string) {
    req, err := http.NewRequest(http.MethodPost, url+"/ship-order", nil)
    _ = err
    _ = req
}
"#;

    let record = extract_syntactic(code, "router.go").expect("Go extraction should succeed");
    assert_eq!(record.endpoints.len(), 1);
    assert_eq!(record.endpoints[0].uri, "/payment/{order_id}");

    let mut typed = models::ir::project::TypedFileRecord::from(record);
    identify(&mut typed);
    assert_eq!(typed.raw_restcalls.len(), 1);
    assert_eq!(typed.raw_restcalls[0].target_uri, "url/ship-order");
}

#[test]
fn extracts_chi_routes() {
    let code = r#"
package main

import "net/http"

func broker(http.ResponseWriter, *http.Request) {}
func submit(http.ResponseWriter, *http.Request) {}
func dynamic(http.ResponseWriter, *http.Request) {}

func routes() http.Handler {
    mux := chi.NewRouter()
    mux.Get("/status", dynamic)
    mux.Head("/status", dynamic)
    mux.Options("/status", dynamic)
    mux.Post("/", broker)
    mux.Post("/handle", submit)
    mux.Method("DELETE", "/items/{id}", http.HandlerFunc(dynamic))
    return mux
}
"#;

    let record = extract_syntactic(code, "routes.go").expect("Go extraction should succeed");
    assert_eq!(record.endpoints.len(), 6);
    assert!(
        record
            .endpoints
            .iter()
            .any(|e| { e.uri == "/status" && e.http_method == models::HttpMethod::GET })
    );
    assert!(
        record
            .endpoints
            .iter()
            .any(|e| e.uri == "/" && e.http_method == models::HttpMethod::POST)
    );
    assert!(
        record
            .endpoints
            .iter()
            .any(|e| e.uri == "/handle" && e.http_method == models::HttpMethod::POST)
    );
    assert!(
        record
            .endpoints
            .iter()
            .any(|e| e.uri == "/items/{id}" && e.http_method == models::HttpMethod::DELETE)
    );
    assert!(
        record
            .endpoints
            .iter()
            .any(|e| e.uri == "/status" && e.http_method == models::HttpMethod::HEAD)
    );
    assert!(
        record
            .endpoints
            .iter()
            .any(|e| e.uri == "/status" && e.http_method == models::HttpMethod::OPTIONS)
    );

    let mut typed = models::ir::project::TypedFileRecord::from(record);
    identify(&mut typed);
    assert!(
        typed.raw_restcalls.is_empty(),
        "route registrations must not become outgoing REST calls: {:?}",
        typed.raw_restcalls
    );
}

#[test]
fn extracts_serve_mux_handle_and_client_methods() {
    let code = r#"
package main

import (
    "fmt"
    "net/http"
)

func httpGetProduct() http.HandlerFunc {
    return func(w http.ResponseWriter, r *http.Request) {}
}

func routes() {
    mux := http.NewServeMux()
    mux.Handle("/get-product", httpGetProduct())
}

type RestClient struct {
    restClient *http.Client
    ProductCatalogService string
}

func (c *RestClient) GetProduct(productID string) {
    url := fmt.Sprintf("http://%s/%s?product_id=%s", c.ProductCatalogService, "get-product", productID)
    _, _ = c.restClient.Get(url)
}
"#;

    let record = extract_syntactic(code, "sample.go").expect("Go extraction should succeed");
    assert!(record.endpoints.iter().any(|e| e.uri == "/get-product"));

    let mut typed = models::ir::project::TypedFileRecord::from(record);
    identify(&mut typed);
    assert!(
        typed
            .raw_restcalls
            .iter()
            .any(|call| call.target_uri.contains("/get-product?product_id="))
    );
}

#[test]
fn extracts_gin_routes() {
    let code = r#"
package main

func checkout() {}

func startRest() {
    router := gin.Default()
    router.POST("/checkout", checkout)
}
"#;

    let record = extract_syntactic(code, "gin.go").expect("Go extraction should succeed");
    assert!(
        record
            .endpoints
            .iter()
            .any(|e| e.uri == "/checkout" && e.http_method == models::HttpMethod::POST)
    );
}

#[test]
fn extracts_gin_group_routes() {
    let code = r#"
package main

func health() {}
func getCustomer() {}
func addItemToBasket() {}

func startRest() {
    router := gin.Default()
    routerGroup := router.Group("/api")
    routerGroup.GET("/customers/:id", getCustomer)
    routerGroup.POST("/customer-basket", addItemToBasket)
    routerGroup.GET("/health", health)
}
"#;

    let record = extract_syntactic(code, "gin_group.go").expect("Go extraction should succeed");
    assert!(
        record
            .endpoints
            .iter()
            .any(|e| e.uri == "/api/customers/:id" && e.http_method == models::HttpMethod::GET)
    );
    assert!(
        record
            .endpoints
            .iter()
            .any(|e| e.uri == "/api/customer-basket" && e.http_method == models::HttpMethod::POST)
    );
    assert!(
        record
            .endpoints
            .iter()
            .any(|e| { e.uri == "/api/health" && e.http_method == models::HttpMethod::GET })
    );
}

#[test]
fn extracts_rabbitmq_and_kafka_message_edges() {
    let code = r#"
package messaging

func publish(ctx any, writer any, producer any, syncProducer any, client any, channel any) {
    _ = writer.WriteMessages(ctx, kafka.Message{Topic: "segment.out"})
    _ = producer.Produce(&ckafka.Message{TopicPartition: ckafka.TopicPartition{Topic: strPtr("confluent.out")}}, nil)
    _ = syncProducer.SendMessage(&sarama.ProducerMessage{Topic: "sarama.out"})
    _ = client.Produce(ctx, &kgo.Record{Topic: "franz.out"})
    _ = channel.Publish("events", "rabbit.out", false, false, amqp.Publishing{})
    _ = channel.QueueDeclare("billing", true, false, false, false, nil)
    _ = channel.QueueBind("billing", "created", "events", false, nil)
}

func consume(reader any, consumer any, partitionConsumer any, group any, channel any) {
    _ = kafka.NewReader(kafka.ReaderConfig{Topic: "segment.in"})
    _ = consumer.SubscribeTopics([]string{"confluent.in", "confluent.retry"}, nil)
    _, _ = partitionConsumer.ConsumePartition("sarama.in", 0, 0)
    _ = group.Consume(ctx, []string{"sarama.group.in"}, handler)
    _ = kgo.SeedTopics("franz.in")
    _, _ = channel.Consume("billing", "", false, false, false, false, nil)
}
"#;

    let record = extract_syntactic(code, "messaging.go").expect("Go extraction should succeed");
    let mut typed = models::ir::project::TypedFileRecord::from(record);
    identify(&mut typed);

    let has_edge = |protocol, role, destination: &str| {
        typed.raw_message_edges.iter().any(|edge| {
            edge.protocol == protocol && edge.role == role && edge.destination == destination
        })
    };

    for destination in ["segment.out", "confluent.out", "sarama.out", "franz.out"] {
        assert!(has_edge(
            models::CommunicationProtocol::Kafka,
            models::MessageRole::Producer,
            destination
        ));
    }
    for destination in [
        "segment.in",
        "confluent.in",
        "confluent.retry",
        "sarama.in",
        "sarama.group.in",
        "franz.in",
    ] {
        assert!(has_edge(
            models::CommunicationProtocol::Kafka,
            models::MessageRole::Consumer,
            destination
        ));
    }
    assert!(has_edge(
        models::CommunicationProtocol::RabbitMq,
        models::MessageRole::Producer,
        "events:rabbit.out"
    ));
    assert!(has_edge(
        models::CommunicationProtocol::RabbitMq,
        models::MessageRole::QueueDeclaration,
        "billing"
    ));
    assert!(has_edge(
        models::CommunicationProtocol::RabbitMq,
        models::MessageRole::Binding,
        "events:created"
    ));
    assert!(has_edge(
        models::CommunicationProtocol::RabbitMq,
        models::MessageRole::Consumer,
        "billing"
    ));
}

#[test]
fn resolves_service_hosts_from_init_assignments() {
    let code = r#"
package main

import (
    "fmt"
    "net/http"
)

const PRODUCT_CATALOG_SERVICE_ADDR = "PRODUCT_CATALOG_SERVICE_ADDR"

var defaultServiceName = map[string]string{
    PRODUCT_CATALOG_SERVICE_ADDR: "product-catalog-service",
}

type RestClient struct {
    restClient *http.Client
    ProductCatalogService string
}

var client = &RestClient{}

func resolveEndpoint(serviceEnv string, port int) string {
    serviceHost := defaultServiceName[serviceEnv]
    service := fmt.Sprintf("%s:%d", serviceHost, port)
    return service
}

func init() {
    client.ProductCatalogService = resolveEndpoint(PRODUCT_CATALOG_SERVICE_ADDR, 60000)
}

func (c *RestClient) GetProduct(productID string) {
    url := fmt.Sprintf("http://%s/%s?product_id=%s", c.ProductCatalogService, "get-product", productID)
    _, _ = c.restClient.Get(url)
}
"#;

    let record = extract_syntactic(code, "sample.go").expect("Go extraction should succeed");
    let mut typed = models::ir::project::TypedFileRecord::from(record);
    identify(&mut typed);
    let uris = typed
        .raw_restcalls
        .iter()
        .map(|call| call.target_uri.clone())
        .collect::<Vec<_>>();
    assert!(
        uris.iter().any(|uri| {
            uri == "http://product-catalog-service:60000/get-product?product_id=productID"
        }),
        "resolved URIs: {uris:?}"
    );
}

#[test]
fn preserves_literal_path_segments_when_resolving_selector_hosts() {
    let code = r#"
package main

import (
    "fmt"
    "net/http"
)

const CART_SERVICE_ADDR = "CART_SERVICE_ADDR"

var defaultServiceName = map[string]string{
    CART_SERVICE_ADDR: "cart-service",
}

type RestClient struct {
    restClient *http.Client
    CartService string
}

var client = &RestClient{}

func getService(serviceEnv string, port int) string {
    serviceHost := defaultServiceName[serviceEnv]
    return fmt.Sprintf("%s:%d", serviceHost, port)
}

func init() {
    client.CartService = getService(CART_SERVICE_ADDR, 60000)
}

func (c *RestClient) GetCart(user_id string) {
    url := fmt.Sprintf("http://%s/%s/user_id/%s", c.CartService, "cart", user_id)
    request, _ := http.NewRequest("GET", url, nil)
    _, _ = c.restClient.Do(request)
    cart := &pb.Cart{}
    _ = cart
}
"#;

    let record = extract_syntactic(code, "sample.go").expect("Go extraction should succeed");
    let mut typed = models::ir::project::TypedFileRecord::from(record);
    identify(&mut typed);
    let uris = typed
        .raw_restcalls
        .iter()
        .map(|call| call.target_uri.clone())
        .collect::<Vec<_>>();
    assert!(
        uris.iter()
            .any(|uri| { uri == "http://cart-service:60000/cart/user_id/user_id" }),
        "resolved URIs: {uris:?}"
    );
}

#[test]
fn extracts_multiple_rest_client_calls() {
    let code = r#"
package main

import (
    "bytes"
    "fmt"
    "net/http"
)

const (
    CART_SERVICE_ADDR = "CART_SERVICE_ADDR"
    PRODUCT_CATALOG_SERVICE_ADDR = "PRODUCT_CATALOG_SERVICE_ADDR"
    PAYMENT_SERVICE_ADDR = "PAYMENT_SERVICE_ADDR"
)

var defaultServiceName = map[string]string{
    CART_SERVICE_ADDR: "cart-service",
    PRODUCT_CATALOG_SERVICE_ADDR: "product-catalog-service",
    PAYMENT_SERVICE_ADDR: "payment-service",
}

type RestClient struct {
    restClient *http.Client
    CartService string
    ProductCatalogService string
    Paymentservice string
}

var client = NewRestClient()

func NewRestClient() *RestClient {
    return &RestClient{restClient: &http.Client{}}
}

func getService(serviceEnv string, port int) string {
    serviceHost := defaultServiceName[serviceEnv]
    return fmt.Sprintf("%s:%d", serviceHost, port)
}

func init() {
    client.CartService = getService(CART_SERVICE_ADDR, 60000)
    client.ProductCatalogService = getService(PRODUCT_CATALOG_SERVICE_ADDR, 60000)
    client.Paymentservice = getService(PAYMENT_SERVICE_ADDR, 60000)
}

func (c *RestClient) GetProduct(productID string) {
    url := fmt.Sprintf("http://%s/%s?product_id=%s", c.ProductCatalogService, "get-product", productID)
    _, _ = c.restClient.Get(url)
}

func (c *RestClient) GetCart(userID string) {
    url := fmt.Sprintf("http://%s/%s/user_id/%s", c.CartService, "cart", userID)
    request, _ := http.NewRequest("GET", url, nil)
    _, _ = c.restClient.Do(request)
}

func (c *RestClient) charge(data []byte) {
    url := fmt.Sprintf("http://%s/%s", c.Paymentservice, "charge")
    _, _ = c.restClient.Post(url, "application/json", bytes.NewBuffer(data))
}
"#;

    let record = extract_syntactic(code, "sample.go").expect("Go extraction should succeed");
    let mut typed = models::ir::project::TypedFileRecord::from(record);
    identify(&mut typed);
    let uris = typed
        .raw_restcalls
        .iter()
        .map(|call| call.target_uri.clone())
        .collect::<Vec<_>>();
    assert!(
        uris.contains(
            &"http://product-catalog-service:60000/get-product?product_id=productID".to_string()
        ),
        "resolved URIs: {uris:?}"
    );
    assert!(
        uris.contains(&"http://cart-service:60000/cart/user_id/userID".to_string()),
        "resolved URIs: {uris:?}"
    );
    assert!(
        uris.contains(&"http://payment-service:60000/charge".to_string()),
        "resolved URIs: {uris:?}"
    );
}

#[test]
fn resolves_constructor_initialized_receiver_fields() {
    let code = r#"
package main

import (
    "fmt"
    "net/http"
)

type Client struct {
    hostURL string
}

func NewCustomerClient() *Client {
    return &Client{
        hostURL: config.AppConfig.CustomerServiceEndpoint,
    }
}

func (c *Client) GetBasketItems(customerID string) {
    resp, _ := http.Get(c.hostURL + fmt.Sprintf("/customers/%v/basketItems", customerID))
    _ = resp
}
"#;

    let record = extract_syntactic(code, "customer_http_client.go")
        .expect("customer_http_client.go should parse");
    let mut typed = models::ir::project::TypedFileRecord::from(record);
    identify(&mut typed);

    let uris = typed
        .raw_restcalls
        .iter()
        .map(|call| call.target_uri.clone())
        .collect::<Vec<_>>();
    assert!(
        uris.contains(
            &"config.AppConfig.CustomerServiceEndpoint/customers/customerID/basketItems"
                .to_string()
        ),
        "resolved URIs: {uris:?}"
    );
}

#[test]
fn does_not_treat_regular_delete_method_calls_as_endpoints() {
    let code = r#"
package api

import "net/http"

const basePath = "/api/v1/stationservice"

func NewRouter(stations *Service) http.Handler {
    mux := http.NewServeMux()
    mux.HandleFunc("DELETE "+basePath+"/stations/{stationsId}", func(writer http.ResponseWriter, request *http.Request) {
        _ = stations.Delete(request.Context(), request.PathValue("stationsId"))
    })
    return mux
}
"#;

    let record = extract_syntactic(code, "router.go").expect("Go extraction should succeed");
    assert_eq!(record.endpoints.len(), 1);
    assert_eq!(
        record.endpoints[0].uri,
        "/api/v1/stationservice/stations/{stationsId}"
    );
}

#[test]
fn skips_generated_go_files() {
    assert!(!should_extract_file(Path::new("service.pb.go")));
    assert!(!should_extract_file(Path::new("service_grpc.pb.go")));
    assert!(!should_extract_file(Path::new("gen/thriftgo/client.go")));
    assert!(should_extract_file(Path::new("service.go")));
}

#[test]
fn uses_import_and_receiver_provenance_for_rest_operations() {
    let code = r#"
package api

import (
    nethttp "net/http"
    "github.com/go-chi/chi/v5"
)

func listItems(nethttp.ResponseWriter, *nethttp.Request) {}

func routes() {
    router := chi.NewRouter()
    router.Get("/items", listItems)
}

func load() {
    store.Get("/cache", listItems)
    _, _ = nethttp.Get("http://inventory-service/items")
}
"#;

    let record = extract_syntactic(code, "api/routes.go").expect("Go extraction should succeed");
    assert_eq!(record.imports.len(), 2);
    assert_eq!(record.endpoints.len(), 1);
    assert_eq!(record.endpoints[0].uri, "/items");

    let mut typed = models::ir::project::TypedFileRecord::from(record);
    identify(&mut typed);
    assert_eq!(typed.raw_restcalls.len(), 1);
    assert_eq!(
        typed.raw_restcalls[0].target_uri,
        "http://inventory-service/items"
    );
}

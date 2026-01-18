# Java REST Calls

There many ways one can call an endpoint from Java, below are those that are most used and those we want to extract.

## HttpClient

- Does not use dependencies, available in Java 11+

```java
HttpClient client = HttpClient.newHttpClient();
HttpRequest request = HttpRequest.newBuilder()
        .uri(URI.create("https://api.example.com/data"))
        .GET()
        .build();

HttpResponse<String> response =
        client.send(request, HttpResponse.BodyHandlers.ofString());

System.out.println(response.body());
```

## Apache HttpClient

- Often used in frameworks internally rather then directly

```java
CloseableHttpClient client = HttpClients.createDefault();
HttpGet request = new HttpGet("https://api.example.com/data");

CloseableHttpResponse response = client.execute(request);
```

## Jakarta EE / MicroProfile

- Less popular outside enterprise

```java
Client client = ClientBuilder.newClient();
String response = client
    .target("https://api.example.com/data")
    .request()
    .get(String.class);
```

## |Spring Framework|

- Not recommended for new code

```java
RestTemplate restTemplate = new RestTemplate();
String result = restTemplate.getForObject(
        "https://api.example.com/data", String.class);
```

## Spring WebFlux

- Preferred solution for Spring today

```java
WebClient client = WebClient.create("https://api.example.com");

String result = client.get()
        .uri("/data")
        .retrieve()
        .bodyToMono(String.class)
        .block();
```

> Declarative clients

## OpenFeign

- Very popular in microservices

```java
@FeignClient(name = "example", url = "https://api.example.com")
public interface ExampleClient {
    @GetMapping("/data")
    String getData();
}
```

## MicroProfile

- Used in Quarkus

```java
@RegisterRestClient
@Path("/data")
public interface ExampleClient {

    @GET
    String getData();
}
```

> Mostly used for Android

## OkHttp

```java
OkHttpClient client = new OkHttpClient();
Request request = new Request.Builder()
        .url("https://api.example.com/data")
        .build();

Response response = client.newCall(request).execute();
```

## Retrofit (Built on OkHttp)

```java
public interface ApiService {
    @GET("data")
    Call<String> getData();
}
```

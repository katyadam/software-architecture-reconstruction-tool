package billing.service.client;

import order.service.model.Order;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.core.ParameterizedTypeReference;
import org.springframework.http.HttpEntity;
import org.springframework.http.HttpHeaders;
import org.springframework.http.HttpMethod;
import org.springframework.http.ResponseEntity;
import org.springframework.stereotype.Service;
import org.springframework.web.client.RestTemplate;
import java.util.List;

@Service
public class OrderClient {

    @Autowired
    private RestTemplate restTemplate;

    private String getServiceUrl(String serviceName) {
        return "http://" + serviceName;
    }

    public List<Order> getAllOrders(HttpHeaders headers) {
        HttpEntity requestEntity = new HttpEntity(headers);
        String order_service_url = getServiceUrl("ts-order-service");
        ResponseEntity<List<Order>> re = restTemplate.exchange(
                order_service_url + "/api/v1/orders/",
                HttpMethod.GET,
                requestEntity,
                new ParameterizedTypeReference<List<Order>>() {});
        return re.getBody();
    }

    public Order createOrder(Order order, HttpHeaders headers) {
        HttpEntity requestEntity = new HttpEntity(order, headers);
        String order_service_url = getServiceUrl("ts-order-service");
        ResponseEntity<Order> re = restTemplate.exchange(
                order_service_url + "/api/v1/orders/",
                HttpMethod.POST,
                requestEntity,
                Order.class);
        return re.getBody();
    }
}

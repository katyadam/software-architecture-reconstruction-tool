import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.HttpEntity;
import org.springframework.http.HttpHeaders;
import org.springframework.http.HttpMethod;
import org.springframework.http.ResponseEntity;
import org.springframework.stereotype.Service;
import org.springframework.web.client.RestTemplate;

@Service
public class RestCallEdgeCases {

    @Autowired
    private RestTemplate restTemplate;

    private String getServiceUrl(String serviceName) {
        return "http://" + serviceName;
    }

    // Edge case 1: HttpMethod.PATCH — previously silently dropped due to missing FromStr arm
    public void updateUserPartial(String userId, Object body, HttpHeaders headers) {
        String user_service_url = getServiceUrl("ts-user-service");
        HttpEntity requestEntity = new HttpEntity(body, headers);
        restTemplate.exchange(
                user_service_url + "/api/v1/users/" + userId,
                HttpMethod.PATCH,
                requestEntity,
                Response.class);
    }

    // Edge case 2: Two REST calls in one method — both must be extracted
    public void syncUserAndOrder(String userId, HttpHeaders headers) {
        String user_service_url = getServiceUrl("ts-user-service");
        String order_service_url = getServiceUrl("ts-order-service");
        HttpEntity requestEntity = new HttpEntity(null, headers);
        restTemplate.exchange(
                user_service_url + "/api/v1/users/" + userId,
                HttpMethod.GET,
                requestEntity,
                Response.class);
        restTemplate.exchange(
                order_service_url + "/api/v1/orders/user/" + userId,
                HttpMethod.GET,
                requestEntity,
                Response.class);
    }

    // Edge case 3: URL with multiple concatenated path parameters
    public void getOrderItem(String orderId, String itemId, HttpHeaders headers) {
        String order_service_url = getServiceUrl("ts-order-service");
        HttpEntity requestEntity = new HttpEntity(null, headers);
        restTemplate.exchange(
                order_service_url + "/api/v1/orders/" + orderId + "/items/" + itemId,
                HttpMethod.GET,
                requestEntity,
                Response.class);
    }
}

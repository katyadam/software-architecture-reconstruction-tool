public class CancelServiceImpl implements CancelService {
    @Autowired
    private RestTemplate restTemplate;

    private String getServiceUrl(String serviceName) {
        return "http://" + serviceName;
    }

    public boolean drawbackMoney(String money, String userId, HttpHeaders headers) {
        CancelServiceImpl.LOGGER.info("[drawbackMoney][Draw Back Money]");

        HttpHeaders newHeaders = getAuthorizationHeadersFrom(headers);
        HttpEntity requestEntity = new HttpEntity(newHeaders);
        String inside_payment_service_url = getServiceUrl("ts-inside-payment-service");
        ResponseEntity<Response> re = restTemplate.exchange(
                inside_payment_service_url + "/api/v1/inside_pay_service/inside_payment/drawback/" + userId + "/" + money,
                HttpMethod.GET,
                requestEntity,
                Response.class);
        Response result = re.getBody();

        return result.getStatus() == 1;
    }

}
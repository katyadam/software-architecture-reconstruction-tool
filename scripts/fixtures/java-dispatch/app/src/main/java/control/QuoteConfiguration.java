package control;

import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;

@Configuration(proxyBeanMethods = false)
public class QuoteConfiguration {
    @Bean
    public InjectedQuote quote() {
        return new InjectedQuote(10);
    }
}

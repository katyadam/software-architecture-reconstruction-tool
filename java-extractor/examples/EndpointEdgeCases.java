package edge.endpoints;

import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

@RestController
@RequestMapping("/api/v1/products")
public class EndpointEdgeCases {

    // Edge case 1: @PatchMapping — PATCH method (enum variant exists but FromStr was missing it)
    @PatchMapping("/{id}")
    public ResponseEntity<?> partialUpdate(@PathVariable String id, @RequestBody Object patch) {
        return ResponseEntity.ok().build();
    }

    // Edge case 2: Nested resource path with multiple path variables
    @GetMapping("/{productId}/reviews/{reviewId}")
    public ResponseEntity<?> getProductReview(
            @PathVariable String productId,
            @PathVariable String reviewId) {
        return ResponseEntity.ok().build();
    }

    // Edge case 3: @RequestMapping without a method attribute — defaults to GET
    // Documents that only shortcut annotations (@GetMapping etc.) carry the HTTP method;
    // bare @RequestMapping produces a GET endpoint by default.
    @RequestMapping("/search")
    public ResponseEntity<?> search(@RequestParam String query) {
        return ResponseEntity.ok().build();
    }
}   

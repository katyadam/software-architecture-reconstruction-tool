# Endpoint-RestCall matching

## Problem

- How to solve a problem finding a REST call correct target endpoint?
- You can match directly (endpointUrl == restCallTargetUrl).
- This is not feasible, often endpoint doesn't contain full url specification.
- Also service can be deployed in a way that it is accessible by multiple URLs - docker network, localhost, etc...

## Solution

- Introduced 2 types of matching
- Each service in configuration can contain multiple urls that it is accesible from.

### Exact Matching

- Using endpoint's service urls with endpoint url and exactly matching RestCall urls formed in a same manner, e.g. service-url/extracted-uri.
- This is classic == but with some sort of URL normalizatiion

### Levenshtein Distance

- Computing Levenshtein Distance for URLs formed the same as was described in "Exact Matching".
- Taking the lowest distance found for specific Endpoint-RestCall pair.

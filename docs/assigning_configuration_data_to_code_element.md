# How is the assigning of configuration metadata to code elements work?

## Solution

1. Extractor assign to each code element a filepath it was extracted from.
2. Synthesizer then fetches configuration and uses it to correctly assign service description to code elements if they need it.

## Previously

Previously the configuration included only service name, because of that the configuration was fetched in extractor and the service name were assigned in the extractor. After finding out that I need to add urls of the service to code elements, I moved assignning service data from extractor to synthesizer. The current state is described in the `Solution` above.

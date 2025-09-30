# Problem

We need that of the particular codebase all elements are extracted and then send to synthesizer.
Due to how synthesizing algorithms work, this needs to be done in a way that synthesizer has all necessary code elements at once.
Sending the extracted data in one API call is not optimal.

# Solution

We want to start with a streaming/batching approach to send all the data from extractor to synthesizer via stream/batches. This should be better then sending one request with huge amount of data.

# Alternative

Another way is to make the synthesizing algorithms work without complete data (all extracted code elements).

# Investigated Solution - Preffered

The extraction can gradually store extracted data into a object storage (S3 compatible storage). After the whole extraction is done, extractor sends just the place of the stored data. Sysnthesizer then loads the data and synthesizes views.

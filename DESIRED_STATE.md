# VOYANTCLAIR

- A tool for automatic analysis of architectures and domain structures of codebases created in multiple languages

## Features

- Creating various views of a codebase
- Analyzing created views to provide feedback to stakeholders
- Providing results analysis of changes made to a codebase and showcasing changes themselves in constructed views
- Integration with Git and API
- Application for managing analysis of codebases and viewing constructed views

## Creation of various Views

- complex process of extraction and synthesizing
- done in Rust using library that provides a fast and memory efficient Context Syntax Tree (CST) creation
- from CST there are extracted various code elements - entities, calls, callables, endpoints, REST calls and imports
- code elements are then send to synthesizer that creates views from them

## Storing the created Views

- used Neo4J graph database to store views (represented as graphs)
- metadata about underlying codebases and views stored in PostgreSQL relational database

## Visualizing the stored Views

- through API, stored views can be retrieved by client
- this client uses modern JavaScript libraries to visualize the view as a directed graph
- graphs can be large, there will be adequate visualization techniques and algorithms to provide as smooth as possible feedback for stakeholders
- providing UI for creating various analysis on top of the stored views and providing their results in adequate form - within the view or as a notification

## Methods of Deployment

### SAAS

- hosted in cloud, providing access through SSO

### On Premise

- providing built software and essential support for its deployment

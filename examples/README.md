# Turbolift Examples

## Kubernetes

Distributing portions of a rust application on Kubernetes was the reason turbolift was made. Internally, turbolift 
uses the derived source code for each image to create a containerized HTTP server. The container is added to a local 
 registry, and a deployment and service is then created that exposes the server to requests from the main program. When 
 the main program completes and the K8s turbolift manager is dropped from memory, it removes the local registry with 
 the container, as well as the deployment and service. 

## Local Queue

The local queue example should never be used in a production application. It's designed to test the core features of 
turbolift (automatically extracting microservices from a rust codebase and running them on an http server),
without any of the platform-specific code for e.g. running on kubernetes. Check this example out if you're interested in 
a bare-bones example turbolift project without any platform-specific specialization. Note: if you're looking to run code 
locally in turbolift instead of using a distribution platform, you should deactivate the distributed turbolift feature 
in your project's `Cargo.toml`. This will let your program run all services locally, e.g. while developing.
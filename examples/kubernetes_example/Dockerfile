# docker build -f examples/kubernetes_example/Dockerfile -t kub .
# docker run -v /var/run/docker.sock:/var/run/docker.sock kub

# set up kind (k8s in docker)
FROM rustlang/rust:nightly
RUN  apt-get update \
  && apt-get install -y wget \
  && apt-get install -y docker.io \
  && rm -rf /var/lib/apt/lists/*
RUN wget -O ./kubectl --show-progress "https://storage.googleapis.com/kubernetes-release/release/$(curl -s https://storage.googleapis.com/kubernetes-release/release/stable.txt)/bin/linux/amd64/kubectl"
RUN chmod +x ./kubectl
RUN wget -O ./kind --show-progress "https://kind.sigs.k8s.io/dl/v0.9.0/kind-$(uname)-amd64"
RUN chmod +x ./kind
COPY ./ turbolift
CMD sh turbolift/examples/kubernetes_example/example_runtime.sh

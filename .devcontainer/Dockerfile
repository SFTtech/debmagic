ARG DISTRO=trixie
FROM mcr.microsoft.com/devcontainers/base:${DISTRO}

# enable deb-src repositories for apt source to work
RUN sed -i -e 's/Types: deb/Types: deb deb-src/g' /etc/apt/sources.list.d/debian.sources
# install everything required for debian package building
RUN apt-get update && apt-get -y install build-essential python3 python3-pip curl debhelper devscripts dh-python
RUN echo "/workspaces/debmagic/src" > /usr/lib/python3/dist-packages/debmagic.pth

# install uv
USER vscode
RUN curl -LsSf https://astral.sh/uv/install.sh | sh
FROM mcr.microsoft.com/devcontainers/base:noble

# install everything required for debian package building
RUN apt-get update && apt-get -y install build-essential python3 python3-pip curl debhelper devscripts dh-python
RUN echo "/workspaces/debmagic/src" > /usr/lib/python3/dist-packages/debmagic.pth

# install uv
USER vscode
RUN curl -LsSf https://astral.sh/uv/install.sh | sh
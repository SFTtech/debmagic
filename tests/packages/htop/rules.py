#!/usr/bin/env python3

from debmagic.v0 import Build, autotools, package

pkg = package(
    preset=autotools,
    maint_options="hardening=+all",
)

if pkg.buildflags.DEB_HOST_ARCH_OS == "linux":
    configure_params = ["--enable-affinity", "--enable-delayacct"]
else:
    configure_params = ["--enable-hwloc"]

# hurd-i386 can open /proc (nothing there) and /proc/ which works
if pkg.buildflags.DEB_HOST_ARCH_OS == "hurd":
    configure_params += ["--with-proc=/proc/"]
else:
    configure_params += ["--enable-sensors"]


def configure(build: Build):
    autotools.configure(
        build,
        ["--enable-openvz", "--enable-vserver", "--enable-unicode"] + configure_params,
    )

pkg.pack()

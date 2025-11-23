#!/usr/bin/env python3

from debmagic.v0 import Build, package
from debmagic.v0 import autotools as autotools_mod
from debmagic.v0 import dh as dh_mod

autotools = autotools_mod.Preset()
dh = dh_mod.Preset()

pkg = package(
    preset=[dh, autotools],
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


@pkg.stage
def configure(build: Build):
    autotools_mod.autoreconf(build)
    autotools.configure(
        build,
        ["--enable-openvz", "--enable-vserver", "--enable-unicode", *configure_params],
    )


@dh.override
def dh_installgsettings(build: Build):
    print("test dh override works :)")
    build.cmd("dh_installgsettings")


@pkg.custom_function
def something_custom(some_param: int, another_param: str = "some default"):
    print(f"you called {some_param=} {another_param=}")


pkg.pack()

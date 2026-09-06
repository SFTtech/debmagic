LA_FILES:=kio_locate.la  _python_module.la  trailing-slash.la dependency_libs.la

%.la: %.txt
	cp "$<" "$@"

all: $(LA_FILES)

clean:
	rm -f $(LA_FILES)

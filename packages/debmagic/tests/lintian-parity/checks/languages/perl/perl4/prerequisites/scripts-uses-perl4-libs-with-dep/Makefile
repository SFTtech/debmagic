all: naughty-script.1

%.1: %
	pod2man --section 1 "$<" > "$@"

clean:
	rm -f naughty-script.1

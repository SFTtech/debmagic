all: pargs
	sed s/pargs/pargs-broken/ pargs > pargs-broken
	sed s/pargs/pargs-section/ pargs > pargs-section
	pod2man --section=1 pargs pargs.1
	pod2man --section=1 pargs-broken pargs-broken.1
	pod2man --section=1 pargs-section pargs-section.1


clean:
	rm -f pargs*.1
	rm -f pargs-*

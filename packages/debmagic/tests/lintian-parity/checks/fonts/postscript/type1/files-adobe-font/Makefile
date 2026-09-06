all: bad.pfb good.pfb badnocredit.pfb badwithadobecode.pfb

%.pfb: %.raw
	t1asm $< > $@

clean:
	rm -f *.pfb


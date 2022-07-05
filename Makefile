.PHONY : all, install, clean
all : 
	cargo build --release

target/release/pid-fan-controller : 
	make all

install: target/release/pid-fan-controller
	cp target/release/pid-fan-controller /usr/local/bin/

clean:
	cargo clean

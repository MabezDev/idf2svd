.PHONY: all
all: clean gpio.json spi.json uart.json timer.json

clean:
	rm -r build/

esp8266-technical_reference_en.pdf:
	mkdir build 2>/dev/null
	wget https://www.espressif.com/sites/default/files/documentation/esp8266-technical_reference_en.pdf -O build/esp8266-technical_reference_en.pdf

appendix.pdf: esp8266-technical_reference_en.pdf
	qpdf --empty --pages build/esp8266-technical_reference_en.pdf 113-116 -- build/appendix.pdf

tabula.jar:
	wget https://github.com/tabulapdf/tabula-java/releases/download/v1.0.3/tabula-1.0.3-jar-with-dependencies.jar -O build/tabula.jar

gpio.json: appendix.pdf tabula.jar
	java -jar build/tabula.jar -p 1 -l -f JSON build/appendix.pdf -o build/gpio.json

spi.json: appendix.pdf tabula.jar
	java -jar build/tabula.jar -p 2 -l -f JSON build/appendix.pdf -o build/spi.json

uart.json: appendix.pdf tabula.jar
	java -jar build/tabula.jar -p 3 -l -f JSON build/appendix.pdf -o build/uart.json

timer.json: appendix.pdf tabula.jar
	java -jar build/tabula.jar -p 4 -l -f JSON build/appendix.pdf -o build/timer.json

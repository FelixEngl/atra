# Atra - The smaller way to crawl

**!!This read me will we reworked in a few days, i am  
currently working on a better version and a wiki for the 
config files.!!**

Atra is a novel web crawling solution, implemented in Rust, designed with
the primary goal of scraping websites as comprehensively
as possible, while ensuring ease of use and accessibility.

## Your first crawl
Download the precompiled executable (coming soon) and run the following command:
- Windows: `./atra.exe single -s test_crawl -d 2 --absolute https://choosealicense.com`
- Linux: `./atra single -s test_crawl -d 2 --absolute https://choosealicense.com`
You will then find a folder `atra_data` on the level of the binary.

## Crawling more
1. Create a file with the name seeds.txt
   - Add a single url per line
   - Put it in the directory with the atra binary
2. Call `./atra.exe --generate-example-config` or `./atra --generate-example-config`
   - Modify the values to meet your needs
   - rename them to `atra.ini` and `crawl.yaml`
3. Call `./atra.exe multi --log-to-file file:seeds.txt` or `./atra multi --log-to-file file:seeds.txt`


## How to build?
In order to build Atra you need [Rust](https://www.rust-lang.org/).

### Windows
After installing Rust you need [LLVM](https://llvm.org/), with the proper 
environment paths set.

### Linux
After installing rust you need `pkg-config`, `libssl-dev`, `clang`, and `llvm` in order to compile Atra. 
You can also use the docker container to build a binary.

Due to the dynamic linking you will need `libc6`, `openssl`, 
and `ca-certificates` installed on you system.

## Why is the crawler named Atra?
The name Atra comes from the Erigone atra, a dwarf spider with a body length of 1.8mm to 2.8mm.
Not only do they play a central role in natural pest control in
agriculture (aphids), but they are also aerial spiders that can
travel long distances by ballooning, also known as kiting. 

More fun spider facts can be found on [Wikipedia](https://en.wikipedia.org/wiki/Erigone_atra).
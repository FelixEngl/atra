# Atra - The smaller way to crawl

**!!This read me will we reworked in a few days. Currently I am working on a better version and a wiki for the 
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

## Config
Atra is configured by using json-configs and environment variables. 
The following table shows the configs written as qualified paths for a json.

| Path                                | format                                                                                         | Explanation                                                                                                                                                                             |
|-------------------------------------|------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| system                              | JSON                                                                                           | Contains all configs regarding the system. Usually caches.                                                                                                                              |
| system.robots_cache_size            | uInt /wo 0; Element Count                                                                      | The cache size of the robots manager. (default: 32)                                                                                                                                     |
| system.web_graph_cache_size         | uInt /wo 0; Element Count                                                                      | The cache size of the webgraph manager (default: 20.000)                                                                                                                                |
| system.max_file_size_in_memory      | uInt; in Byte                                                                                  | Max size of the files stored in memory. (default: 100MB). <br/> If set to 0 nothing will be stored in memory.                                                                           |
| system.log_level                    | String; Enum (see [Log Level](#Log-Level))                                                     | The log level of the crawler. (default: Info)                                                                                                                                           |
| system.log_to_file                  | boolean                                                                                        | Log to a file and not to console. (default: false)                                                                                                                                      |
| paths                               | JSON                                                                                           | Contains all configs regarding the paths.                                                                                                                                               |
| paths.root                          | String; Path                                                                                   | The root path where the application runs. (default: "./atra_data")                                                                                                                      |
| paths.directories                   | JSON                                                                                           |                                                                                                                                                                                         |
| paths.directories.database          | String; Path                                                                                   | Path to the database directory. (default: _root_/rocksdb)                                                                                                                               |
| paths.directories.big_files         | String; Path                                                                                   | Path to the big files directory. (default: _root_/big_files)                                                                                                                            |
| paths.files                         | JSON                                                                                           |                                                                                                                                                                                         |
| paths.files.queue                   | String; Path                                                                                   | Path to the queue file (if one is needed) (default: _root_/queue.tmp)                                                                                                                   |
| paths.files.blacklist               | String; Path                                                                                   | Path to the blacklist (default: _root_/blacklist.txt)                                                                                                                                   |
| paths.files.web_graph               | String; Path                                                                                   | Path to the web graph generated by atra (default: _root_/web_graph.ttl)                                                                                                                 |
| session                             | JSON                                                                                           | The config of the session                                                                                                                                                               |
| session.service                     | String                                                                                         | The name of the service (default: "atra")                                                                                                                                               |
| session.collection                  | String                                                                                         | The name of the collection created (default: "unnamed")                                                                                                                                 |
| session.crawl_job_id                | uInt                                                                                           | The crawl job id. To differentiate for the same service and collection.                                                                                                                 |
| session.warc_compression_level      | uInt                                                                                           | - unused -                                                                                                                                                                              |
| crawl                               | JSON                                                                                           |                                                                                                                                                                                         |
| crawl.user_agent                    | String; Enum (see [User Agents](#User-Agents))                                                 | The user agent used by the crawler.  (default: Default)                                                                                                                                 |
| crawl.respect_robots_txt            | boolean                                                                                        | Respect robots.txt file and not scrape not allowed files. This may slow down crawls if<br/>robots.txt file has a delay included. (default: true)                                        |
| crawl.respect_nofollow              | boolean                                                                                        | Respect the nofollow attribute during the link extraction (default: true)                                                                                                               |
| crawl.crawl_embedded_data           | boolean                                                                                        | Extract links to embedded data like audio/video files for the crawl-queue (default: false)                                                                                              |
| crawl.crawl_javascript              | boolean                                                                                        | Extract links to/from javascript files for the crawl-queue (default: true)                                                                                                              |
| crawl.crawl_onclick_by_heuristic    | boolean                                                                                        | Try to extract links from tags with onclick attribute for the crawl-queue (default: false)                                                                                              |
| crawl.apply_gdbr_filter_if_possible | boolean                                                                                        | Tries to apply an gdbr filter, if one was properly configured.                                                                                                                          |
| crawl.store_only_html_in_warc       | boolean                                                                                        | Only store html-files in the warc                                                                                                                                                       |
| crawl.max_file_size                 | uInt/null; in Byte                                                                             | The maximum size to download. If null there is no limit. (default: null)                                                                                                                |
| crawl.max_robots_age                | String/null; "`[whole_seconds].[whole_nanoseconds]`"                                           | The maximum age of a cached robots.txt. If null, it never gets too old.                                                                                                                 |
| crawl.ignore_sitemap                | boolean                                                                                        | Prevent including the sitemap links with the crawl. (default: false)                                                                                                                    |
| crawl.subdomains                    | boolean                                                                                        | Allow sub-domains. (default: false)                                                                                                                                                     |
| crawl.cache                         | boolean                                                                                        | Cache the page following HTTP caching rules. (default: false)                                                                                                                           |
| crawl.use_cookies                   | boolean                                                                                        | Use cookies (default: false)                                                                                                                                                            |
| crawl.cookies                       | JSON/null; (see [Cookie Settings](#Cookie-Settings))                                           | Domain bound cookie config. (default: null)                                                                                                                                             |
| crawl.headers                       | JSON/null; ``{"- header_name -": "- header_value -"}``                                         | Headers to include with requests. (default: null)                                                                                                                                       |
| crawl.proxies                       | List<String>; ``["- proxy -", "- proxy -"]``                                                   | Use proxy list for performing network request. (default: null)                                                                                                                          |
| crawl.tld                           | boolean                                                                                        | Allow all tlds for domain. (default: false)                                                                                                                                             |
| crawl.delay                         | String; "`[whole_seconds].[whole_nanoseconds]`"                                                | Polite crawling delay (default: 1 second)                                                                                                                                               |
| crawl.budget                        | JSON/null; (see [Crawl Budget](#Crawl-Budget))                                                 | The budget settings for this crawl.                                                                                                                                                     |
| crawl.max_queue_age                 | uInt                                                                                           | How often can we fail to crawl an entry in the queue until it is dropped? (0 means never drop) (default: 20)                                                                            |
| crawl.redirect_limit                | uInt                                                                                           | The max redirections allowed for request. (default: 5 like Google-Bot)                                                                                                                  |
| crawl.redirect_policy               | String; Enum (see [Redirection Policy](#Redirection-Policy))                                   | The redirect policy type to use. (default: Loose)                                                                                                                                       |
| crawl.accept_invalid_certs          | boolean                                                                                        | Dangerously accept invalid certficates (default: false)                                                                                                                                 |
| crawl.link_extractors               | JSON; ``[- Command -, - Command -]`` (see [Link Extractor Settings](#Link-Extractor-Settings)) | A custom configuration of extractors. (default: see [Extractor Settings](#Extractor-Settings))                                                                                          |
| crawl.decode_big_files_up_to        | uInt/null; in Byte                                                                             | If this value is set Atra tries to decode and process files that are only downloaded as<br/>blob but do not overstep this provided size (in Bytes).<br/>Null means off. (default: null) |
| crawl.use_default_stopwords         | boolean                                                                                        | If this is set all stopwords inclide the default stopwords known to atra (drfault: true)                                                                                                |
| crawl.stopword_registry             | JSON/null; (see [Stopword Registry](#Stopword-Registry))                                       | Used to configure the global registry for stopwords.                                                                                                                                    |
| crawl.gbdr                          | JSON/null; (see [GDBR Filter](#GBDR-Filter))                                                   | Used to configure the SVM for filtering GBRS. The model used is the L2R_L2LOSS_SVR.                                                                                                     |
| crawl.chrome_settings               | - unused -                                                                                     | - unused -                                                                                                                                                                              |

### Log Level
| Level | Explanation                                        |
|-------|----------------------------------------------------|
| Off   | Logging off                                        |
| Error | Log only errors                                    |
| Warn  | Log errors and warnings                            |
| Info  | Log errors, warnings and infos                     |
| Debug | Log errors, warnings, infos and debug informations |
| Trace | Log everything.                                    |

### User Agents
| Name    | Value                         | Explanation                                    |
|---------|-------------------------------|------------------------------------------------|
| Spoof   | "Spoof"                       | Uses a static agent that looks like a browser. |
| Default | "Default"                     | Uses "Crawler/Atra/-version-"                  |
| Custom  | ``{ "Custom": "-content-" }`` | Uses a custom user agent                       |

### Cookie Settings
| Sub-Path   | Value                                              | Explanation                                                                               |
|------------|----------------------------------------------------|-------------------------------------------------------------------------------------------|
| default    | String                                             | Cookie string to use for network requests e.g.: "foo=bar; Domain=blog.spider"             |
| per_domain | JSON/null; ``{"- domain -": "- cookie string -"}`` | A map between domains and cookie string.<br/>A domain is e.g.: "ebay.de" or an IP address |

Exemplary entry in a JSON:
````json
{
   "cookies": {
      "default": "foo=bar; Domain=blog.spider",
      "per_host": {
         "ebay.de": "foo=cat; Domain=blog.spider2"
      }
   }
}
````

### Crawl Budget
| Sub-Path | Value                                                     | Explanation                                                                                 |
|----------|-----------------------------------------------------------|---------------------------------------------------------------------------------------------|
| default  | JSON;BudgetSetting; see [Budget Setting](#Budget-Setting) | The default budget setting for a crawl.                                                     |
| per_host | JSON/null; ``{"- domain/host -": - BudgetSetting - }``    | A map between domains and budget settings.<br/>A domain is e.g.: "ebay.de" or an IP address |

Exemplary entry in a JSON:
````json
{
   "budget": {
      "default": {
         "depth_on_website": 2,
         "depth": 1,
         "recrawl_interval": null,
         "request_timeout": null
      },
      "per_host": {
         "ebay.de": {
            "depth": 3,
            "recrawl_interval": "360.000000000",
            "request_timeout": null
         }
      }
   }
}
````

### Budget Setting
Budget settings exists in 3 different kinds:
- SeedOnly: Only crawls the seed domains
- Normal: Crawls the seed and follows external links
- Absolute: Crawls the seed and follows external links, but only follows until a specific amout of jumps is reached.

The kind is decided by the presence of the fields. As described in the following table.

| Sub-Path         | used in                    | Value                                                | Explanation                                                                   |
|------------------|----------------------------|------------------------------------------------------|-------------------------------------------------------------------------------|
| depth_on_website | SeedOnly, Normal           | uInt                                                 | The max depth to crawl on a website.                                          |
| depth            | Normal, Absolute           | uInt                                                 | The maximum depth of websites, outgoing from the seed.                        |
| recrawl_interval | SeedOnly, Normal, Absolute | String/null; "`[whole_seconds].[whole_nanoseconds]`" | Crawl interval (if set to null crawl only once)   (default: null)             |
| request_timeout  | SeedOnly, Normal, Absolute | String/null; "`[whole_seconds].[whole_nanoseconds]`" | Request max timeout per page. Set to null to disable. (default: 15.000000000) |


### Redirection Policy
| Name   | Value     | Explanation                                                                   |
|--------|-----------|-------------------------------------------------------------------------------|
| Loose  | "Spoof"   | A loose policy that allows all request up to the redirect limit.              |
| Strict | "Default" | A strict policy only allowing request that match the domain set for crawling. |

### Link Extractor Settings
The extractor settings are a list of commands.

| Sub-Path         | Value                                        | Explanation                                            |
|------------------|----------------------------------------------|--------------------------------------------------------|
| extractor_method | String; Enum (see [Apply When](#Apply-When)) | The method used to extract something.                  |
| apply_when       | String; Enum (see [Apply When](#Apply-When)) | The maximum depth of websites, outgoing from the seed. |

Example:
````json
{
   "link_extractor": [
      {
         "extractor_method": "HtmlV1",
         "apply_when": "IfSuitable"
      },
      {
         "extractor_method": "JSV1",
         "apply_when": "IfSuitable"
      },
      {
         "extractor_method": "PlainText",
         "apply_when": "IfSuitable"
      },
      {
         "extractor_method": "RawV1",
         "apply_when": "IfSuitable"
      }
   ]
}
````

#### Link Extractor Method

| Name      | Value                                         | Explanation                                                                                                                    |
|-----------|-----------------------------------------------|--------------------------------------------------------------------------------------------------------------------------------|
| HtmlV1    | "HtmlV1"/"HTML_v1"                            | Extracts links from an HTML. Can respect NO_FOLLOW and is capable of resolving must of the common references of HTML.          |
| JSV1      | "JSV1"/"js_v1"/"JavaScript_v1"/"JS_v1"        | Extracts links from JavaScript by searching for href identifiers.                                                              |
| PlainText | "PlainText"/"PlainText_v1"/"PT_v1"/"Plain_v1" | Extracts links from a plaintext by using linkify. [link](https://crates.io/crates/linkify)                                     |
| RawV1     | "RawV1"/"RAW_v1"                              | Tries to extract links from raw bytes by searching for http:// and https:// and then recovering following texts heuristically. |


#### Apply When
Decides when to apply a link-extractor on some kind of data.

| Name       | Value        | Explanation                                                  |
|------------|--------------|--------------------------------------------------------------|
| Always     | "Always"     | Always applies this extractor.                               |
| IfSuitable | "IfSuitable" | Only applies the extractor iff the file is of a fitting type |
| Fallback   | "Fallback"   | If everything fails, try this extractor.                     |


### Stopword Registry
Consists of a list of stopword repository configurations, can be one of the following:

| Sub-Path         | Value                | Explanation                                                                                                                                                                                                                                  |
|------------------|----------------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| with_iso_default | bool                 | If this is set, the registry uses the default iso stopwords in addition to the provided stopword. (default: false)<br/> If neither a directory nor a file with language is set, it activates the default stopwords as independent stopwords. |
| dir/directory    | Path/Null            | Defines a folder containing txt-files named after like <iso-lang>.txt. The files contains in each line a specific stopword.                                                                                                                  |
| file             | Path/Null            | Points to a txt file. The files contains in each line a specific stopword. The language id defined by the required "language" field in the config.                                                                                           |
| language         | String; Isolang Name | A language hint for file based stopword lists.                                                                                                                                                                                               |


### GBDR Filter
| Sub-Path    | Value                                                                              | Explanation                                                    |
|-------------|------------------------------------------------------------------------------------|----------------------------------------------------------------|
| default     | JSON/Null; IdentifierConfig; see [GBDR Identifier Config](#GBDR-Identifier-Config) | The default gdbr filter for the crawler.                       |
| by_language | JSON/null; ``{"- Iso Lang -": - IdentifierConfig - }``                             | A map between languages and language specific gdbr identifier. |

#### GBDR Identifier Config

| Sub-Path         | Value                                                       | Explanation                                                                   |
|------------------|-------------------------------------------------------------|-------------------------------------------------------------------------------|
| threshold        | f64                                                         | Sets the minimum score needed for a sucessfull SVM prediction. (default: 0.1) | 
| filter_threshold | f64                                                         | Sets the score needed for a high confident SVM prediction. (default: 0.5)     |
| filter_by        | String; FilterMode; see [GBDR FilterMode](#GBDR-FilterMode) | Configures what score is used to identify the correct node in the html.       |
| svm              | JSON; SvmConfig; see [SVM Config](#SVM-Config)              | Configures the SVM                                                            |

#### GBDR FilterMode
| Name           | Value            | Explanation                                                                                  |
|----------------|------------------|----------------------------------------------------------------------------------------------|
| OnScore        | "OnScore"        | Identify the target node by the score of the current node.                                   |
| OnMaxScore     | "OnMaxScore"     | Identify the target node by the maximum score in one of the children.                        |
| OnAverageScore | "OnAverageScore" | Calculate the average between the score and max score in the child. The result will be used. |

### SVM Config

The SVM can be configured in three ways:
- Load: Tries to load a pretrained model
- Train: Tries to train a model
- All: Tries to load a model, if it fails it tries to train one.

| Sub-Path            | used in          | Value                                                      | Explanation                                                                   |
|---------------------|------------------|------------------------------------------------------------|-------------------------------------------------------------------------------|
| language            | Load, Train, All | String; Iso Language                                       | Sets the minimum score needed for a sucessfull SVM prediction. (default: 0.1) | 
| retrain_if_possible | All              | bool/Null                                                  | Sets the score needed for a high confident SVM prediction. (default: 0.5)     |
| tf                  | Train, All       | String/Null;TF; see [TF](#TF)                              | Configures what score is used to identify the correct node in the html.       |
| idf                 | Train, All       | String/Null;IDF; see [IDF](#IDF)                           | Configures the SVM                                                            |
| tf_idf_data         | Train, All       | Path/Null; see [SVM Data Formats](#SVM-Data-Formats)       | Configures the SVM                                                            |
| train_data          | Train, All       | Path/Null; see [SVM Data Formats](#SVM-Data-Formats)       | Configures the SVM                                                            |
| test_data           | Load, Train, All | Path/Null; see [SVM Data Formats](#SVM-Data-Formats)       | Configures the SVM                                                            |
| trained_svm         | Load, All        | Path/Null                                                  | Configures the SVM                                                            |
| normalize_tokens    | Train, All       | bool/null                                                  | Configures the SVM                                                            |
| filter_stopwords    | Train, All       | bool/null                                                  | Configures the SVM                                                            |
| stemmer             | Train, All       | String; StemmerName; see [Stemmer Names](#Stemmer-Names)   | Configures the SVM                                                            |
| parameters          | Train, All       | JSON; SvmParameters; see [SVM Parameters](#SVM-Parameters) | Configures the SVM                                                            |
| min_doc_length      | Load, Train, All | uInt/null                                                  | Configures the SVM                                                            |
| min_vector_length   | Load, Train, All | uInt/null                                                  | Configures the SVM                                                            |

#### SVM Parameters

| Sub-Path          | used in        | Value                                                                     | Explanation                                                                   |
|-------------------|----------------|---------------------------------------------------------------------------|-------------------------------------------------------------------------------|
| epsilon           | L2R_L2LOSS_SVR | f64/null                                                                  | Sets the minimum score needed for a sucessfull SVM prediction. (default: 0.1) | 
| cost              | L2R_L2LOSS_SVR | f64/null                                                                  | Sets the score needed for a high confident SVM prediction. (default: 0.5)     |
| p                 | L2R_L2LOSS_SVR | f64/null                                                                  | regression loss sensitivity                                                   |
| nu                |                | f64/null                                                                  | Configures the SVM                                                            |
| cost_penalty      | L2R_L2LOSS_SVR | Array<[Int, f64]>/null; ```json {"cost_penalty": [[1, 0.5], [2, 3.4]]}``` | Configures the SVM                                                            |
| initial_solutions | L2R_L2LOSS_SVR | Array<f64>/null                                                           | Configures the SVM                                                            |
| bias              | L2R_L2LOSS_SVR | f64/null                                                                  | Configures the SVM                                                            |
| regularize_bias   | L2R_L2LOSS_SVR | bool                                                                      | Configures the SVM                                                            |

#### TF
See https://en.wikipedia.org/wiki/Tf%E2%80%93idf for the explanations:
- Binary | $`{0,1}`$
- RawCount | $`f_{t,d}`$
- TermFrequency | $`f_{t,d} \Bigg/ {\sum_{t' \in d}{f_{t',d}}}`$
- LogNormalization | $`\log (1 + f_{t,d})`$
- DoubleNormalization | $`0.5 + 0.5 \cdot \frac { f_{t,d} }{\max_{\{t' \in d\}} {f_{t',d}}}`$

#### IDF
See https://en.wikipedia.org/wiki/Tf%E2%80%93idf for the explanations:
$`n_t = |\{d \in D: t \in d\}|`$

- Unary | 1
- InverseDocumentFrequency | $`\log \frac {N} {n_t} = - \log \frac {n_t} {N}`$
- InverseDocumentFrequencySmooth | $`\log \left( \frac {N} {1 + n_t}\right)+ 1`$
- InverseDocumentFrequencyMax | $`\log \left(\frac {\max_{\{t' \in d\}} n_{t'}} {1 + n_t}\right)`$
- ProbabilisticInverseDocumentFrequency | $`\log  \frac {N - n_t} {n_t}`$

##### German GDBR SVM Config
This parameters proved very robust for german gdbr recognition.

````json
{
   "language": "Deu",
   "tf": "TermFrequency",
   "idf": "InverseDocumentFrequency",
   "normalize_tokens": true,
   "filter_stopwords": true,
   "stemmer": "German",
   "parameters": {
      "epsilon": 0.0003,
      "p": 0.1,
      "cost": 10.0
   },
   "min_doc_length": 5,
   "min_vector_length": 5
}
````

#### SVM Data Formats

### Stemmer Names
Stemmers are available in the following languages, the name for the parameters are the same:
- Arabic
- Danish
- Dutch
- English
- Finnish
- French
- German
- Greek
- Hungarian
- Italian
- Norwegian
- Portuguese
- Romanian
- Russian
- Spanish
- Swedish
- Tamil
- Turkish
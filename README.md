# 🌐 Robust Minimal Web Crawler in Rust

A robust yet minimal web crawler implemented in Rust, utilizing various libraries and aiming for scalability and extensibility.

![Web Crawler](images/spider-crawling-world.png)

## Dependencies

- **`reqwest`**: Used for making HTTP requests efficiently, with a client per worker thread.
- **`scraper`**: Used to process the HTML DOM and extract links, and everything else to do with HTML.
- **`tokio`**: For efficient asynchronous programming.

## Features

- [x] **Dependency between links**: We store the parent-child dependency between links.
- [x] **Multiple Workers**: Visit links through multiple asynchronous workers (a client per worker).
- [x] **Image Scraping**: Download images found along the way.
- [ ] **General Scraping Support (Upcoming)**: Support any data scraping in the links.
- [ ] **Distributed Database Integration (Upcoming)**: Aims to integrate support for distributed databases.
- [ ] **Grafana Metrics (Upcoming)**: Plans to add metrics support using Grafana for better insights.

## Installation and Usage

To use this web crawler, simply check out the repository, build, and run with:

```bash
cargo b --release
./target/release/rusty_crawler --help
```

## Contributing

Feel free to contribute by opening issues or submitting pull requests!

## License

This project is licensed under the MIT License.

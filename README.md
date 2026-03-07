```bash
cargo run -p cli --release -- \
  -p ./my_project_directory \
  -c ./config.json \
  -f ./constants.json \
  -o ./output_directory
```

```bash
cargo run -p cli --release -- \
  -p ../empaia \
  -c ../empaia/conf.json \
  -f ../empaia/constants.json \
  -o ./empaia-sar-output
```

```bash
cargo run -p cli --release -- \
  -p ../train-ticket \
  -c ./local-train-ticket-config.json \
  -o ./train-ticket-sar-output
```
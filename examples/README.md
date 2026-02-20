# Examples

## ReAct Agent

A file assistant that demonstrates the ReAct (Reasoning + Acting) pattern built with Polaris. See [agents.md](../docs/reference/agents.md) for the pattern specification.

### Tools

| Tool | Description |
|------|-------------|
| `list_files` | List files in a directory |
| `read_file` | Read the contents of a file |
| `write_file` | Write content to a file |

All paths are relative to the working directory, which acts as a sandbox. The agent cannot access files outside this directory.

### Running

Run the following commands from the `examples/` directory:

```bash
export ANTHROPIC_API_KEY=your-key

# Run the agent
cargo run --bin react -- <working_dir> <query>

# Example
cargo run -p examples --bin react -- ./sandbox "What files are here?"
```

## Sessions

Demonstrates resource persistence across runs. Reuses the ReAct agent and adds a `SessionPlugin` that saves/loads conversation history to a JSON file.

### Running

```bash
# First run — ask a question
cargo run -p examples --bin sessions -- test1 ./sandbox "What files are here?"

# Second run — conversation history is restored
cargo run -p examples --bin sessions -- test1 ./sandbox "Which file did I ask about?"
```

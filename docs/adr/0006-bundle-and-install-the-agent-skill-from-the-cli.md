# Bundle And Install The Agent Skill From The CLI

Neowright will include a bundled agent skill and expose `neowright skills install` to install it into `.agents/skills/neowright/`, with `--global` installing to the user-level `.agents` directory. Embedding the skill in the released binary ensures agents can learn Neowright workflows without requiring the source repository or a separate package.

# References

## Primary Sources

### Repository & Documentation
- **GitHub**: https://github.com/SWE-agent/mini-swe-agent
- **Documentation**: https://mini-swe-agent.com
- **PyPI**: https://pypi.org/project/mini-swe-agent/
- **Tutorial** (building a minimal agent from scratch): https://minimal-agent.com

### Key Source Files
- **Agent class** (~100 lines): [`src/minisweagent/agents/default.py`](https://github.com/SWE-agent/mini-swe-agent/blob/main/src/minisweagent/agents/default.py)
- **Environment**: [`src/minisweagent/environments/local.py`](https://github.com/SWE-agent/mini-swe-agent/blob/main/src/minisweagent/environments/local.py)
- **Model wrapper**: [`src/minisweagent/models/litellm_model.py`](https://github.com/SWE-agent/mini-swe-agent/blob/main/src/minisweagent/models/litellm_model.py)
- **Run script**: [`src/minisweagent/run/hello_world.py`](https://github.com/SWE-agent/mini-swe-agent/blob/main/src/minisweagent/run/hello_world.py)
- **Default config**: [`src/minisweagent/config/default.yaml`](https://github.com/SWE-agent/mini-swe-agent/blob/main/src/minisweagent/config/default.yaml)
- **Roulette model**: [`src/minisweagent/models/extra/roulette.py`](https://github.com/SWE-agent/mini-swe-agent/blob/main/src/minisweagent/models/extra/roulette.py)
- **Exceptions**: [`src/minisweagent/exceptions.py`](https://github.com/SWE-agent/mini-swe-agent/blob/main/src/minisweagent/exceptions.py)
- **Tool call parsing**: [`src/minisweagent/models/utils/actions_toolcall.py`](https://github.com/SWE-agent/mini-swe-agent/blob/main/src/minisweagent/models/utils/actions_toolcall.py)

### Documentation Pages
- **Control flow**: https://mini-swe-agent.com/latest/advanced/control_flow/
- **FAQ (incl. "why no shell session")**: https://mini-swe-agent.com/latest/faq/
- **Quick start**: https://mini-swe-agent.com/latest/quickstart/
- **Cookbook**: https://mini-swe-agent.com/latest/advanced/cookbook/
- **YAML configuration**: https://mini-swe-agent.com/latest/advanced/yaml_configuration/
- **Local models**: https://mini-swe-agent.com/latest/models/local_models/
- **v2 migration guide**: https://mini-swe-agent.com/latest/advanced/v2_migration/

## Blog Posts & Announcements

- **Gemini 3 Pro reaches 74% on SWE-bench Verified**: https://x.com/KLieret/status/1991164693839270372
- **Model roulette blog post** (randomly switching GPT-5 + Sonnet 4): https://www.swebench.com/post-250820-mini-roulette.html
- **GPT-5 evaluation blog post**: https://www.swebench.com/post-250808-gpt5.html

## Academic Papers

### SWE-agent (the predecessor)

```bibtex
@inproceedings{yang2024sweagent,
  title={{SWE}-agent: Agent-Computer Interfaces Enable Automated Software Engineering},
  author={John Yang and Carlos E Jimenez and Alexander Wettig and Kilian Lieret
          and Shunyu Yao and Karthik R Narasimhan and Ofir Press},
  booktitle={The Thirty-eighth Annual Conference on Neural Information Processing Systems},
  year={2024},
  url={https://arxiv.org/abs/2405.15793}
}
```

### SWE-bench

```bibtex
@inproceedings{jimenez2024swebench,
  title={{SWE}-bench: Can Language Models Resolve Real-World {GitHub} Issues?},
  author={Carlos E. Jimenez and John Yang and Alexander Wettig and Shunyu Yao
          and Kexin Pei and Ofir Press and Karthik Narasimhan},
  booktitle={The Twelfth International Conference on Learning Representations},
  year={2024},
  url={https://arxiv.org/abs/2310.06770}
}
```

### ReAct (foundational technique)

```bibtex
@inproceedings{yao2023react,
  title={{ReAct}: Synergizing Reasoning and Acting in Language Models},
  author={Shunyu Yao and Jeffrey Zhao and Dian Yu and Nan Du and Izhak Shafran
          and Karthik Narasimhan and Yuan Cao},
  booktitle={The Eleventh International Conference on Learning Representations},
  year={2023},
  url={https://arxiv.org/abs/2210.03629}
}
```

## Related Projects (SWE-bench Family)

| Project | Description | Link |
|---------|-------------|------|
| **SWE-bench** | The benchmark | https://github.com/SWE-bench/SWE-bench |
| **SWE-agent** | Full-featured coding agent (predecessor) | https://github.com/SWE-agent/SWE-agent |
| **SWE-ReX** | Remote execution framework | https://github.com/SWE-agent/SWE-ReX |
| **SWE-smith** | Synthetic training data generation | https://github.com/SWE-bench/SWE-smith |
| **CodeClash** | LM head-to-head code competition | https://github.com/codeclash-ai/codeclash |
| **sb-cli** | SWE-bench CLI | https://github.com/SWE-bench/sb-cli |

## Key People

- **Kilian Lieret** — Lead developer of mini-SWE-agent (Princeton → Stanford)
- **Carlos E. Jimenez** — Co-creator of SWE-bench and SWE-agent
- **John Yang** — Co-creator of SWE-agent
- **Ofir Press** — Principal investigator (Princeton → Stanford)
- **Shunyu Yao** — ReAct author, SWE-bench contributor
- **Karthik Narasimhan** — Faculty advisor (Princeton)
- **Alexander Wettig** — SWE-agent contributor

## Community

- **Slack**: https://join.slack.com/t/swe-bench/shared_invite/zt-36pj9bu5s-o3_yXPZbaH2wVnxnss1EkQ
- **SWE-bench Leaderboard**: https://www.swebench.com
- **Twitter/X**: https://twitter.com/SWEbench
- **YouTube**: https://www.youtube.com/@SWE-bench

## Dependencies

- **LiteLLM**: https://github.com/BerriAI/litellm — Universal LM API wrapper
- **Pydantic**: https://docs.pydantic.dev/ — Data validation and settings
- **Jinja2**: https://jinja.palletsprojects.com/ — Template rendering
- **Typer**: https://typer.tiangolo.com/ — CLI interface
- **OpenRouter**: https://openrouter.ai/ — Multi-provider LM routing
- **Portkey**: https://portkey.ai/ — LM gateway

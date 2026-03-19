# Droid — References

> Comprehensive list of public sources for Factory/Droid research.

## Official Resources

| Resource | URL | Description |
|----------|-----|-------------|
| Factory Homepage | https://factory.ai | Main product page, overview of Droids |
| Factory Docs | https://docs.factory.ai | Onboarding, configuration, integration guides |
| Enterprise Page | https://factory.ai/enterprise | Enterprise features, testimonials, integrations |
| Security Page | https://factory.ai/security | Security measures, compliance, Droid Shield |
| Pricing | https://factory.ai/pricing | Plans, token allocations, feature tiers |
| News/Blog | https://factory.ai/news | Product announcements, case studies, engineering posts |

## Documentation

| Topic | URL | Key Content |
|-------|-----|-------------|
| Onboarding Overview | https://docs.factory.ai/onboarding | Four-step setup process |
| SSO Setup | https://docs.factory.ai/onboarding/creating-your-factory/sso-setup | SAML/OIDC identity provider integration |
| Basic Integrations | https://docs.factory.ai/onboarding/creating-your-factory/basic-integrations | SCM, PM, knowledge, incident integrations |
| `.droid.yaml` Config | https://docs.factory.ai/onboarding/configuring-your-factory/droid-yaml-configuration | Review configuration, auto-review, guidelines |
| GitHub Cloud Integration | https://docs.factory.ai/onboarding/integrating-with-your-engineering-system/github-cloud | GitHub repository integration steps |

## Blog Posts and Announcements

| Title | URL | Date | Category |
|-------|-----|------|----------|
| Factory Analytics | https://factory.ai/news/factory-analytics | March 11, 2026 | Product |
| Factory Expands to London | https://factory.ai/news/factory-london | March 2026 | Company |
| Wipro Partnership | https://factory.ai/news/wipro | January 28, 2026 | Partnership |

## Case Studies

| Customer | URL | Key Metrics |
|----------|-----|-------------|
| Chainguard | https://factory.ai/case-studies/chainguard | 2-week session, 6 repos, 80 packages |

## Testimonials

| Person | Company | Quote |
|--------|---------|-------|
| Aman Mulani, Full-Stack Engineer | Clari | "Factory has nearly doubled my productivity" |
| Josh Wolf, Staff Engineer | Chainguard | "Compaction is just that good" |

## Benchmark References

| Benchmark | Version | Droid Rank | Score | Model |
|-----------|---------|------------|-------|-------|
| Terminal-Bench | 2.0 | #6 | 77.3% | GPT-5.3-Codex |
| Terminal-Bench | 2.0 | #16 | 69.9% | Claude Opus 4.6 |
| Terminal-Bench | 2.0 | #23 | 64.9% | GPT-5.2 |
| Terminal-Bench | 1.0 | #5 | 58.8% | Claude Opus 4.1 |

## Partnerships

| Partner | Type | Details |
|---------|------|---------|
| Wipro (NYSE: WIT) | Strategic partnership + investment | Integration into WEGA platform; Wipro Ventures investor; rollout to tens of thousands of engineers |

## Integrations (Confirmed)

GitHub, GitLab, Jira, Linear, Notion, Sentry, PagerDuty, Slack, Asana, Microsoft Teams, Codecov, Google Cloud, Confluence, Google Drive, Azure, Docker, DX, CircleCI

## Security & Compliance

| Standard | Status |
|----------|--------|
| ISO 42001 | Adopted (among first worldwide) |
| SOC 2 Type I | Achieved |
| AES-256 (at rest) | Implemented |
| TLS 1.2+ (in transit) | Implemented |
| Single-tenant VPC | Standard for enterprise |

## Key People

| Name | Role | Source |
|------|------|--------|
| Matan Grinberg | Co-Founder & CEO | Wipro partnership announcement |
| Sandhya Arun | CTO, Wipro (partner) | Wipro partnership announcement |
| Ali Wasti | Managing Partner, Wipro Ventures | Wipro partnership announcement |
| Josh Wolf | Staff Engineer, Chainguard (customer) | Case study |
| Matt Moore | CTO, Chainguard (customer) | Referenced in case study |

## Research Notes

- Factory is closed-source; no public GitHub repository for the agent itself.
- Documentation is gated behind authentication for most detailed content (onboarding steps, feature details).
- The news/blog index pages require JavaScript rendering and don't expose article lists in static HTML.
- Factory Analytics blog post is the most technically detailed public resource about the platform's internals.
- The Chainguard case study provides the best real-world validation of the compaction engine.
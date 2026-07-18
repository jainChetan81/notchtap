# mcp server setup — brightdata + product-research plugins

## servers installed

### 1. brightdata mcp server (`@brightdata/mcp` v2.11.0)
**location:** `mcp-servers/brightdata/`
**tools:** 55+ including serp search, web scraping, batch operations, structured datasets, browser automation.
**config:** `mcp-servers/brightdata/mcp-config.json`

### 2. product-research plugin wrapper
**location:** `mcp-servers/product-research-plugins/`
**tools:**
- `brightdata-plugin:price-comparison` — finds product prices across retailers, ranks offers.
- `brightdata-plugin:competitive-intel` — vendor comparison, pricing tiers, feature matrices.
**config:** `mcp-servers/product-research-plugins/mcp-config.json`

## combined config
`mcp-servers/mcp-config-all.json` — register both servers in your mcp client.

## how to register in kimi work
paste the contents of `mcp-config-all.json` into your kimi work mcp server settings, then restart.

## skills that now work
- `product-research` — requires the wrapper plugin tools above.
- `consensus` — already available (pal reviewers).

## token
brightdata api token is wired into both configs. zones `mcp_unlocker` and `mcp_browser` verified active.

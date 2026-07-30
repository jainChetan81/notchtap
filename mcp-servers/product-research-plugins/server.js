const {FastMCP} = require('fastmcp');
const axios = require('axios');
const z = require('zod');

const api_token = process.env.BRIGHTDATA_API_KEY || process.env.API_TOKEN;
const unlocker_zone = process.env.WEB_UNLOCKER_ZONE || 'mcp_unlocker';

if (!api_token) {
  console.error('[product-research-plugins] error: BRIGHTDATA_API_KEY or API_TOKEN required');
  process.exit(1);
}

const api_headers = () => ({
  authorization: `Bearer ${api_token}`,
  'content-type': 'application/json',
});

async function web_unlocker_request(url, opts = {}) {
  const res = await axios({
    url: 'https://api.brightdata.com/request',
    method: 'POST',
    data: {
      url,
      zone: unlocker_zone,
      format: 'raw',
      data_format: opts.data_format || 'markdown',
      ...opts.extra,
    },
    headers: api_headers(),
    responseType: 'text',
    timeout: 60000,
  });
  return res.data;
}

function extract_prices_from_markdown(md) {
  // crude but effective extraction of price-like patterns and nearby context
  const lines = md.split('\n');
  const offers = [];
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    // match currency symbols + numbers
    const price_match = line.match(/(?:₹|rs\.?|inr|\$|€|£)\s*[\d,]+(?:\.\d{2})?/i);
    if (price_match) {
      const raw = price_match[0];
      const price = parseFloat(raw.replace(/[^\d.]/g, ''));
      const currency = raw.match(/₹|rs\.?|inr/i) ? '₹' :
                       raw.match(/\$/) ? '$' :
                       raw.match(/€/) ? '€' :
                       raw.match(/£/) ? '£' : '₹';
      // look for seller/retailer in surrounding lines
      let context = line;
      if (i > 0) context = lines[i - 1] + ' | ' + context;
      if (i < lines.length - 1) context += ' | ' + lines[i + 1];
      // guess retailer from context
      const retailer_match = context.match(/(amazon|flipkart|walmart|ebay|bestbuy|etsy|swiggy\s*instamart|blinkit|zara|homedepot|target)/i);
      const retailer = retailer_match ? retailer_match[1].toLowerCase() : 'unknown';
      // guess in-stock
      const in_stock = !/out of stock|unavailable|sold out/i.test(context);
      if (price > 0) {
        offers.push({ price, currency, retailer, in_stock, context: line.trim() });
      }
    }
  }
  // dedupe by retailer+price
  const seen = new Set();
  return offers.filter(o => {
    const key = `${o.retailer}-${o.price}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

function extract_features_from_markdown(md) {
  const lines = md.split('\n');
  const features = [];
  const tiers = [];
  for (const line of lines) {
    const tier_match = line.match(/(free|starter|basic|pro|premium|enterprise|business|team)\s*[:\-]?\s*(?:₹|\$|€|£)?\s*[\d,]+/i);
    if (tier_match) {
      const price_match = line.match(/(?:₹|\$|€|£)\s*[\d,]+(?:\.\d{2})?/);
      tiers.push({
        name: tier_match[1],
        price_line: line.trim(),
        price: price_match ? price_match[0] : null,
      });
    }
    if (/^[-*]\s+/.test(line) && line.length > 10 && line.length < 200) {
      features.push(line.replace(/^[-*]\s+/, '').trim());
    }
  }
  return { features: features.slice(0, 20), tiers: tiers.slice(0, 10) };
}

const server = new FastMCP({
  name: 'product-research-plugins',
  version: '1.0.0',
});

server.addTool({
  name: 'brightdata-plugin:price-comparison',
  description: 'Finds where a product is sold, for how much, and whether it is in stock. '
    + 'Searches across amazon, flipkart, walmart, google shopping, and other retailers. '
    + 'Region-aware for India with pincode support. Returns a ranked buy-recommendation table.',
  annotations: {
    title: 'Price Comparison',
    readOnlyHint: true,
    openWorldHint: true,
  },
  parameters: z.object({
    product: z.string().describe('Product name or search query'),
    pincode: z.string().optional().describe('India pincode for region-aware pricing (optional)'),
    budget: z.number().optional().describe('Maximum price budget'),
    retailers: z.array(z.string()).optional().describe('Specific retailers to check (e.g., ["amazon", "flipkart"])'),
  }),
  execute: async ({product, pincode, budget, retailers}) => {
    const results = {
      product,
      pincode: pincode || null,
      budget: budget || null,
      offers: [],
      recommendation: null,
      sources: [],
    };

    // search google shopping
    const geo = pincode ? `&gl=in` : '';
    const query = encodeURIComponent(product);
    const shopping_url = `https://www.google.com/search?tbm=shop&q=${query}${geo}`;
    try {
      const md = await web_unlocker_request(shopping_url, { data_format: 'markdown' });
      results.sources.push({ source: 'google_shopping', url: shopping_url });
      const offers = extract_prices_from_markdown(md);
      results.offers.push(...offers);
    } catch (e) {
      results.sources.push({ source: 'google_shopping', url: shopping_url, error: e.message });
    }

    // search amazon
    try {
      const amazon_url = `https://www.amazon.in/s?k=${query}`;
      const md = await web_unlocker_request(amazon_url, { data_format: 'markdown' });
      results.sources.push({ source: 'amazon', url: amazon_url });
      const offers = extract_prices_from_markdown(md).map(o => ({...o, retailer: 'amazon'}));
      results.offers.push(...offers);
    } catch (e) {
      results.sources.push({ source: 'amazon', url: `https://www.amazon.in/s?k=${query}`, error: e.message });
    }

    // search flipkart if india/pincode present or no region specified
    if (!pincode || pincode.startsWith('5') || pincode.startsWith('6')) {
      try {
        const flipkart_url = `https://www.flipkart.com/search?q=${query}`;
        const md = await web_unlocker_request(flipkart_url, { data_format: 'markdown' });
        results.sources.push({ source: 'flipkart', url: flipkart_url });
        const offers = extract_prices_from_markdown(md).map(o => ({...o, retailer: 'flipkart'}));
        results.offers.push(...offers);
      } catch (e) {
        results.sources.push({ source: 'flipkart', url: `https://www.flipkart.com/search?q=${query}`, error: e.message });
      }
    }

    // filter by requested retailers
    if (retailers && retailers.length > 0) {
      const allowed = retailers.map(r => r.toLowerCase());
      results.offers = results.offers.filter(o => allowed.includes(o.retailer));
    }

    // filter by budget
    if (budget) {
      results.offers = results.offers.filter(o => o.price <= budget);
    }

    // sort by price ascending
    results.offers.sort((a, b) => a.price - b.price);

    // pick recommendation
    const in_stock = results.offers.filter(o => o.in_stock);
    if (in_stock.length > 0) {
      const best = in_stock[0];
      results.recommendation = `buy from ${best.retailer} at ${best.currency}${best.price}`;
    } else if (results.offers.length > 0) {
      results.recommendation = `lowest price: ${results.offers[0].retailer} at ${results.offers[0].currency}${results.offers[0].price} (check stock)`;
    } else {
      results.recommendation = 'no offers found — try a more specific product name';
    }

    return JSON.stringify(results, null, 2);
  },
});

server.addTool({
  name: 'brightdata-plugin:competitive-intel',
  description: 'Competitive intelligence for b2b tools and services. '
    + 'Compares vendors, extracts pricing tiers, builds feature matrices, and identifies strengths/weaknesses.',
  annotations: {
    title: 'Competitive Intelligence',
    readOnlyHint: true,
    openWorldHint: true,
  },
  parameters: z.object({
    query: z.string().describe('Tool, service, or category to research (e.g., "project management software")'),
    vendors: z.array(z.string()).optional().describe('Specific vendor names to compare (optional)'),
    focus: z.enum(['pricing', 'features', 'all']).optional().default('all').describe('What to focus on'),
  }),
  execute: async ({query, vendors, focus}) => {
    const results = {
      query,
      vendors: vendors || [],
      focus,
      comparisons: [],
      feature_matrix: {},
      pricing_summary: [],
      sources: [],
    };

    // search for comparison pages or vendor pricing pages
    const search_queries = [
      `${query} pricing comparison`,
      `${query} vs alternative`,
    ];

    for (const sq of search_queries) {
      try {
        const url = `https://www.google.com/search?q=${encodeURIComponent(sq)}`;
        const md = await web_unlocker_request(url, { data_format: 'markdown' });
        results.sources.push({ source: 'google_search', query: sq, url });
        const {features, tiers} = extract_features_from_markdown(md);
        if (tiers.length > 0) {
          results.pricing_summary.push(...tiers);
        }
        if (features.length > 0) {
          features.forEach(f => {
            results.feature_matrix[f] = results.feature_matrix[f] || 'mentioned';
          });
        }
      } catch (e) {
        results.sources.push({ source: 'google_search', query: sq, error: e.message });
      }
    }

    // if specific vendors given, scrape their pricing pages
    if (vendors && vendors.length > 0) {
      for (const vendor of vendors) {
        const slug = vendor.toLowerCase().replace(/\s+/g, '');
        const pricing_urls = [
          `https://${slug}.com/pricing`,
          `https://${slug}.com/plans`,
          `https://${slug}.io/pricing`,
          `https://www.${slug}.com/pricing`,
        ];
        for (const purl of pricing_urls) {
          try {
            const md = await web_unlocker_request(purl, { data_format: 'markdown' });
            results.sources.push({ source: vendor, url: purl });
            const {features, tiers} = extract_features_from_markdown(md);
            results.comparisons.push({
              vendor,
              url: purl,
              tiers: tiers.slice(0, 5),
              features: features.slice(0, 10),
            });
            break; // stop after first successful page
          } catch (e) {
            // try next url
          }
        }
      }
    }

    // dedupe pricing summary
    const seen = new Set();
    results.pricing_summary = results.pricing_summary.filter(t => {
      const key = `${t.name}-${t.price}`;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });

    return JSON.stringify(results, null, 2);
  },
});

console.error('[product-research-plugins] starting wrapper server...');
server.start({transportType: 'stdio'});

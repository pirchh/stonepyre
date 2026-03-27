CREATE INDEX IF NOT EXISTS idx_portfolios_character
ON portfolio.portfolios(character_id);

CREATE INDEX IF NOT EXISTS idx_holdings_portfolio
ON portfolio.holdings(portfolio_id);

CREATE INDEX IF NOT EXISTS idx_transactions_portfolio_ts
ON portfolio.transactions(portfolio_id, ts DESC);

CREATE INDEX IF NOT EXISTS idx_transactions_company
ON portfolio.transactions(company_id);
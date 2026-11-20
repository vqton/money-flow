CREATE TABLE accounts (
    code VARCHAR(10) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    account_class INTEGER NOT NULL, -- Circular 99 Classification
    regime VARCHAR(10) NOT NULL,    -- 'VAS99' or 'VAS133'
    is_active BOOLEAN DEFAULT TRUE
);

CREATE TABLE journal_entries (
    id UUID PRIMARY KEY,
    debit_account VARCHAR(10) NOT NULL REFERENCES accounts(code),
    credit_account VARCHAR(10) NOT NULL REFERENCES accounts(code),
    amount DECIMAL(19, 4) NOT NULL,
    description TEXT,
    -- Circular 91 Compliance Fields
    invoice_reference VARCHAR(50), 
    invoice_type VARCHAR(20) CHECK(invoice_type IN ('Coded', 'Uncoded', 'CashRegister')),
    correction_type VARCHAR(20) DEFAULT 'None' CHECK(correction_type IN ('None', 'Adjustment', 'Replacement')),
    -- Circular 99 GMT Support
    tax_type VARCHAR(20) DEFAULT 'Standard' CHECK(tax_type IN ('Standard', 'GMT_TopUp')),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

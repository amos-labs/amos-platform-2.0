-- Seed integrations: QuickBooks, Stripe, GoDaddy, Shopify, Gmail, Google Drive, Google Sheets
-- Ported from agent_marketing seed data with operations stored in available_actions JSONB

-- ═══════════════════════════════════════════════════════════════════════════
-- 1. STRIPE (Payment)
-- ═══════════════════════════════════════════════════════════════════════════
INSERT INTO integrations (name, connector_type, endpoint_url, status, available_actions, metadata)
VALUES (
    'Stripe',
    'payment',
    'https://api.stripe.com',
    'disconnected',
    '[
        {
            "operation_id": "stripe.test_connection",
            "name": "Test Connection",
            "description": "Test if your Stripe API key is valid",
            "http_method": "GET",
            "path_template": "/v1/balance",
            "requires_confirmation": false,
            "request_schema": {"type": "object", "properties": {}}
        },
        {
            "operation_id": "stripe.list_customers",
            "name": "List Customers",
            "description": "Returns a list of your customers",
            "http_method": "GET",
            "path_template": "/v1/customers",
            "pagination_strategy": "cursor",
            "requires_confirmation": false,
            "request_schema": {
                "type": "object",
                "properties": {
                    "limit": {"type": "integer", "description": "Number of customers to return (1-100)", "minimum": 1, "maximum": 100, "default": 10},
                    "starting_after": {"type": "string", "description": "Cursor for pagination"},
                    "email": {"type": "string", "description": "Filter by email address"}
                }
            }
        },
        {
            "operation_id": "stripe.create_customer",
            "name": "Create Customer",
            "description": "Creates a new customer object",
            "http_method": "POST",
            "path_template": "/v1/customers",
            "requires_confirmation": true,
            "request_schema": {
                "type": "object",
                "required": ["email"],
                "properties": {
                    "email": {"type": "string", "format": "email", "description": "Customer email address"},
                    "name": {"type": "string", "description": "Customer full name"},
                    "description": {"type": "string", "description": "Arbitrary description"},
                    "phone": {"type": "string", "description": "Customer phone number"},
                    "metadata": {"type": "object", "description": "Set of key-value pairs"}
                }
            }
        },
        {
            "operation_id": "stripe.list_charges",
            "name": "List Charges",
            "description": "Returns a list of charges",
            "http_method": "GET",
            "path_template": "/v1/charges",
            "pagination_strategy": "cursor",
            "requires_confirmation": false,
            "request_schema": {
                "type": "object",
                "properties": {
                    "limit": {"type": "integer", "minimum": 1, "maximum": 100, "default": 10},
                    "customer": {"type": "string", "description": "Filter by customer ID"}
                }
            }
        },
        {
            "operation_id": "stripe.list_invoices",
            "name": "List Invoices",
            "description": "Returns a list of invoices",
            "http_method": "GET",
            "path_template": "/v1/invoices",
            "pagination_strategy": "cursor",
            "requires_confirmation": false,
            "request_schema": {
                "type": "object",
                "properties": {
                    "limit": {"type": "integer", "minimum": 1, "maximum": 100, "default": 10},
                    "customer": {"type": "string", "description": "Filter by customer ID"},
                    "status": {"type": "string", "enum": ["draft", "open", "paid", "uncollectible", "void"]}
                }
            }
        },
        {
            "operation_id": "stripe.list_subscriptions",
            "name": "List Subscriptions",
            "description": "Returns a list of subscriptions",
            "http_method": "GET",
            "path_template": "/v1/subscriptions",
            "pagination_strategy": "cursor",
            "requires_confirmation": false,
            "request_schema": {
                "type": "object",
                "properties": {
                    "limit": {"type": "integer", "minimum": 1, "maximum": 100, "default": 10},
                    "customer": {"type": "string", "description": "Filter by customer ID"},
                    "status": {"type": "string", "enum": ["active", "past_due", "canceled", "unpaid", "trialing"]}
                }
            }
        }
    ]'::jsonb,
    '{
        "slug": "stripe",
        "category": "payment",
        "auth_type": "basic_auth",
        "icon_url": "https://cdn.brandfolder.io/KGT2DTA4/at/8gkvgs86vw4gv4x48878kh4/Stripe_icon_-_square.svg",
        "documentation_url": "https://stripe.com/docs/api",
        "description": "Accept payments, manage subscriptions, and track revenue",
        "auth_config": {
            "auth_method": "basic",
            "username_label": "API Key",
            "username_placeholder": "sk_test_... or sk_live_...",
            "username_help_text": "Enter your Stripe secret key. Stripe uses the API key as username in Basic Auth.",
            "password_required": false,
            "test_endpoint": "/v1/customers?limit=1",
            "setup_instructions": "Get your API key from the Stripe Dashboard under Developers > API Keys"
        },
        "rate_limits": {"default": 100, "per": "second"},
        "api_version": "2020-08-27"
    }'::jsonb
)
ON CONFLICT DO NOTHING;

-- ═══════════════════════════════════════════════════════════════════════════
-- 2. QUICKBOOKS ONLINE (Accounting)
-- ═══════════════════════════════════════════════════════════════════════════
INSERT INTO integrations (name, connector_type, endpoint_url, status, available_actions, metadata)
VALUES (
    'QuickBooks Online',
    'payment',
    'https://quickbooks.api.intuit.com/v3',
    'disconnected',
    '[
        {
            "operation_id": "quickbooks.test_connection",
            "name": "Test Connection",
            "description": "Test if your QuickBooks OAuth token is valid",
            "http_method": "GET",
            "path_template": "/company/{companyId}/companyinfo/{companyId}",
            "requires_confirmation": false,
            "request_schema": {"type": "object", "properties": {}}
        },
        {
            "operation_id": "quickbooks.get_company_info",
            "name": "Get Company Info",
            "description": "Retrieve company information",
            "http_method": "GET",
            "path_template": "/company/{companyId}/companyinfo/{companyId}",
            "requires_confirmation": false,
            "request_schema": {"type": "object", "properties": {}}
        },
        {
            "operation_id": "quickbooks.list_customers",
            "name": "List Customers",
            "description": "Retrieve a list of customers using QuickBooks Query Language",
            "http_method": "GET",
            "path_template": "/company/{companyId}/query",
            "pagination_strategy": "offset",
            "requires_confirmation": false,
            "request_schema": {
                "type": "object",
                "required": ["query"],
                "properties": {
                    "query": {"type": "string", "description": "SQL-like query (e.g. SELECT * FROM Customer)", "default": "SELECT * FROM Customer"},
                    "startPosition": {"type": "integer", "minimum": 1, "default": 1},
                    "maxResults": {"type": "integer", "minimum": 1, "maximum": 1000, "default": 100}
                }
            }
        },
        {
            "operation_id": "quickbooks.create_invoice",
            "name": "Create Invoice",
            "description": "Create a new invoice in QuickBooks",
            "http_method": "POST",
            "path_template": "/company/{companyId}/invoice",
            "requires_confirmation": true,
            "request_schema": {
                "type": "object",
                "required": ["Line", "CustomerRef"],
                "properties": {
                    "CustomerRef": {"type": "object", "required": ["value"], "properties": {"value": {"type": "string", "description": "Customer ID"}}},
                    "Line": {"type": "array", "minItems": 1, "items": {"type": "object", "required": ["Amount", "DetailType"], "properties": {"Amount": {"type": "number"}, "Description": {"type": "string"}, "DetailType": {"type": "string", "enum": ["SalesItemLineDetail"]}}}},
                    "DueDate": {"type": "string", "format": "date", "description": "Invoice due date (YYYY-MM-DD)"},
                    "DocNumber": {"type": "string", "description": "Invoice number"}
                }
            }
        },
        {
            "operation_id": "quickbooks.create_payment",
            "name": "Create Payment",
            "description": "Record a payment from a customer",
            "http_method": "POST",
            "path_template": "/company/{companyId}/payment",
            "requires_confirmation": true,
            "request_schema": {
                "type": "object",
                "required": ["CustomerRef", "TotalAmt"],
                "properties": {
                    "CustomerRef": {"type": "object", "required": ["value"], "properties": {"value": {"type": "string", "description": "Customer ID"}}},
                    "TotalAmt": {"type": "number", "description": "Total payment amount"}
                }
            }
        },
        {
            "operation_id": "quickbooks.list_invoices",
            "name": "List Invoices",
            "description": "Query invoices using QuickBooks Query Language",
            "http_method": "GET",
            "path_template": "/company/{companyId}/query",
            "pagination_strategy": "offset",
            "requires_confirmation": false,
            "request_schema": {
                "type": "object",
                "required": ["query"],
                "properties": {
                    "query": {"type": "string", "description": "SQL-like query", "default": "SELECT * FROM Invoice"},
                    "startPosition": {"type": "integer", "minimum": 1, "default": 1},
                    "maxResults": {"type": "integer", "minimum": 1, "maximum": 1000, "default": 100}
                }
            }
        },
        {
            "operation_id": "quickbooks.list_accounts",
            "name": "List Accounts",
            "description": "Query chart of accounts",
            "http_method": "GET",
            "path_template": "/company/{companyId}/query",
            "pagination_strategy": "offset",
            "requires_confirmation": false,
            "request_schema": {
                "type": "object",
                "required": ["query"],
                "properties": {
                    "query": {"type": "string", "default": "SELECT * FROM Account"}
                }
            }
        }
    ]'::jsonb,
    '{
        "slug": "quickbooks",
        "category": "accounting",
        "auth_type": "oauth2",
        "icon_url": "https://quickbooks.intuit.com/etc/designs/qb-core/graphics/favicon.ico",
        "documentation_url": "https://developer.intuit.com/app/developer/qbo/docs/api/accounting/all-entities/account",
        "description": "Accounting software for invoicing, expenses, and financial reporting",
        "auth_config": {
            "authorize_url": "https://appcenter.intuit.com/connect/oauth2",
            "token_url": "https://oauth.platform.intuit.com/oauth2/v1/tokens/bearer",
            "scopes": ["com.intuit.quickbooks.accounting"],
            "use_basic_auth": true,
            "callback_params": ["realmId"],
            "setup_instructions": "Create an OAuth app at https://developer.intuit.com/app/developer/myapps"
        },
        "rate_limits": {"per_minute": 500, "concurrent_requests": 10},
        "api_version": "v3",
        "minor_version": "65",
        "sandbox_url": "https://sandbox-quickbooks.api.intuit.com/v3",
        "production_url": "https://quickbooks.api.intuit.com/v3"
    }'::jsonb
)
ON CONFLICT DO NOTHING;

-- ═══════════════════════════════════════════════════════════════════════════
-- 3. GODADDY (DNS / Custom Domains)
-- ═══════════════════════════════════════════════════════════════════════════
INSERT INTO integrations (name, connector_type, endpoint_url, status, available_actions, metadata)
VALUES (
    'GoDaddy',
    'custom',
    'https://api.godaddy.com',
    'disconnected',
    '[
        {
            "operation_id": "godaddy.list_domains",
            "name": "List Domains",
            "description": "Returns all domains owned by the authenticated user",
            "http_method": "GET",
            "path_template": "/v1/domains",
            "pagination_strategy": "offset",
            "requires_confirmation": false,
            "request_schema": {
                "type": "object",
                "properties": {
                    "limit": {"type": "integer", "description": "Maximum number of domains to return", "default": 100},
                    "marker": {"type": "string", "description": "Marker for pagination"}
                }
            }
        },
        {
            "operation_id": "godaddy.get_domain",
            "name": "Get Domain Details",
            "description": "Returns detailed information about a specific domain",
            "http_method": "GET",
            "path_template": "/v1/domains/{domain}",
            "requires_confirmation": false,
            "request_schema": {
                "type": "object",
                "required": ["domain"],
                "properties": {"domain": {"type": "string", "description": "Domain name to retrieve"}}
            }
        },
        {
            "operation_id": "godaddy.get_dns_records",
            "name": "Get DNS Records",
            "description": "Returns all DNS records for a domain",
            "http_method": "GET",
            "path_template": "/v1/domains/{domain}/records",
            "requires_confirmation": false,
            "request_schema": {
                "type": "object",
                "required": ["domain"],
                "properties": {
                    "domain": {"type": "string", "description": "Domain name"},
                    "type": {"type": "string", "description": "Filter by record type (A, AAAA, CNAME, MX, TXT, etc.)"},
                    "name": {"type": "string", "description": "Filter by record name"}
                }
            }
        },
        {
            "operation_id": "godaddy.add_dns_record",
            "name": "Add DNS Record",
            "description": "Adds a new DNS record to a domain",
            "http_method": "PATCH",
            "path_template": "/v1/domains/{domain}/records",
            "requires_confirmation": true,
            "request_schema": {
                "type": "object",
                "required": ["domain", "records"],
                "properties": {
                    "domain": {"type": "string", "description": "Domain name"},
                    "records": {"type": "array", "items": {"type": "object", "required": ["type", "name", "data"], "properties": {"type": {"type": "string", "enum": ["A","AAAA","CNAME","MX","TXT","NS","SRV","CAA"]}, "name": {"type": "string"}, "data": {"type": "string"}, "ttl": {"type": "integer", "default": 3600}}}}
                }
            }
        },
        {
            "operation_id": "godaddy.replace_dns_records",
            "name": "Replace DNS Records",
            "description": "Replaces all DNS records of a specific type for a domain",
            "http_method": "PUT",
            "path_template": "/v1/domains/{domain}/records/{type}",
            "requires_confirmation": true,
            "request_schema": {
                "type": "object",
                "required": ["domain", "type", "records"],
                "properties": {
                    "domain": {"type": "string"},
                    "type": {"type": "string", "enum": ["A","AAAA","CNAME","MX","TXT","NS","SRV","CAA"]},
                    "records": {"type": "array", "items": {"type": "object", "required": ["name", "data"], "properties": {"name": {"type": "string"}, "data": {"type": "string"}, "ttl": {"type": "integer", "default": 3600}}}}
                }
            }
        },
        {
            "operation_id": "godaddy.delete_dns_record",
            "name": "Delete DNS Record",
            "description": "Deletes a specific DNS record from a domain",
            "http_method": "DELETE",
            "path_template": "/v1/domains/{domain}/records/{type}/{name}",
            "requires_confirmation": true,
            "request_schema": {
                "type": "object",
                "required": ["domain", "type", "name"],
                "properties": {
                    "domain": {"type": "string"},
                    "type": {"type": "string", "enum": ["A","AAAA","CNAME","MX","TXT","NS","SRV","CAA"]},
                    "name": {"type": "string", "description": "Record name (use @ for root domain)"}
                }
            }
        }
    ]'::jsonb,
    '{
        "slug": "godaddy",
        "category": "dns",
        "auth_type": "api_key",
        "icon_url": "https://www.godaddy.com/favicon.ico",
        "documentation_url": "https://developer.godaddy.com/doc",
        "description": "Manage DNS records for custom domains. Auto-configure CNAME records for landing pages and email sending.",
        "auth_config": {
            "auth_method": "sso-key",
            "api_key_label": "API Key",
            "api_secret_label": "API Secret",
            "header_name": "Authorization",
            "header_template": "sso-key {api_key}:{api_secret}",
            "test_endpoint": "/v1/domains",
            "setup_instructions": "1. Go to https://developer.godaddy.com/keys\n2. Create a new API Key (Production or OTE for testing)\n3. Copy your API Key and Secret"
        },
        "rate_limits": {"default": 60, "per": "minute"},
        "supports_ote": true,
        "ote_base_url": "https://api.ote-godaddy.com"
    }'::jsonb
)
ON CONFLICT DO NOTHING;

-- ═══════════════════════════════════════════════════════════════════════════
-- 4. SHOPIFY (eCommerce)
-- ═══════════════════════════════════════════════════════════════════════════
INSERT INTO integrations (name, connector_type, endpoint_url, status, available_actions, metadata)
VALUES (
    'Shopify',
    'custom',
    'https://{shop_domain}/admin/api/2024-01',
    'disconnected',
    '[
        {
            "operation_id": "shopify.test_connection",
            "name": "Test Connection",
            "description": "Test if your Shopify access token is valid",
            "http_method": "GET",
            "path_template": "/shop.json",
            "requires_confirmation": false,
            "request_schema": {"type": "object", "properties": {}}
        },
        {
            "operation_id": "shopify.list_products",
            "name": "List Products",
            "description": "Retrieves a list of products",
            "http_method": "GET",
            "path_template": "/products.json",
            "pagination_strategy": "page",
            "requires_confirmation": false,
            "request_schema": {
                "type": "object",
                "properties": {
                    "limit": {"type": "integer", "minimum": 1, "maximum": 250, "default": 50},
                    "page": {"type": "integer", "minimum": 1, "default": 1},
                    "product_type": {"type": "string", "description": "Filter by product type"},
                    "vendor": {"type": "string", "description": "Filter by vendor"},
                    "status": {"type": "string", "enum": ["active", "archived", "draft"]}
                }
            }
        },
        {
            "operation_id": "shopify.get_product",
            "name": "Get Product",
            "description": "Retrieves a single product by ID",
            "http_method": "GET",
            "path_template": "/products/{product_id}.json",
            "requires_confirmation": false,
            "request_schema": {
                "type": "object",
                "required": ["product_id"],
                "properties": {"product_id": {"type": "integer", "description": "Product ID"}}
            }
        },
        {
            "operation_id": "shopify.list_orders",
            "name": "List Orders",
            "description": "Retrieves a list of orders",
            "http_method": "GET",
            "path_template": "/orders.json",
            "pagination_strategy": "page",
            "requires_confirmation": false,
            "request_schema": {
                "type": "object",
                "properties": {
                    "limit": {"type": "integer", "minimum": 1, "maximum": 250, "default": 50},
                    "status": {"type": "string", "enum": ["open", "closed", "cancelled", "any"], "default": "any"},
                    "financial_status": {"type": "string", "enum": ["authorized", "pending", "paid", "partially_paid", "refunded", "voided", "any"]}
                }
            }
        },
        {
            "operation_id": "shopify.list_customers",
            "name": "List Customers",
            "description": "Retrieves a list of customers",
            "http_method": "GET",
            "path_template": "/customers.json",
            "pagination_strategy": "page",
            "requires_confirmation": false,
            "request_schema": {
                "type": "object",
                "properties": {
                    "limit": {"type": "integer", "minimum": 1, "maximum": 250, "default": 50}
                }
            }
        },
        {
            "operation_id": "shopify.get_inventory_levels",
            "name": "Get Inventory Levels",
            "description": "Retrieves inventory levels for items",
            "http_method": "GET",
            "path_template": "/inventory_levels.json",
            "requires_confirmation": false,
            "request_schema": {
                "type": "object",
                "properties": {
                    "inventory_item_ids": {"type": "string", "description": "Comma-separated inventory item IDs"},
                    "location_ids": {"type": "string", "description": "Comma-separated location IDs"}
                }
            }
        }
    ]'::jsonb,
    '{
        "slug": "shopify",
        "category": "ecommerce",
        "auth_type": "api_key",
        "icon_url": "https://cdn.shopify.com/shopifycloud/brochure/assets/brand-assets/shopify-logo-primary-logo.png",
        "documentation_url": "https://shopify.dev/docs/api/admin-rest",
        "description": "Manage your online store, products, orders, and customers",
        "auth_config": {
            "auth_method": "header",
            "auth_field_name": "X-Shopify-Access-Token",
            "requires_shop_domain": true,
            "setup_instructions": "Create a private app in your Shopify admin to get an access token"
        },
        "rate_limits": {"default": 2, "burst": 40, "per": "second"},
        "api_version": "2024-01"
    }'::jsonb
)
ON CONFLICT DO NOTHING;

-- ═══════════════════════════════════════════════════════════════════════════
-- 5. GMAIL (Google Suite - Communication)
-- ═══════════════════════════════════════════════════════════════════════════
INSERT INTO integrations (name, connector_type, endpoint_url, status, available_actions, metadata)
VALUES (
    'Gmail',
    'email',
    'https://gmail.googleapis.com/gmail/v1',
    'disconnected',
    '[
        {
            "operation_id": "gmail.test_connection",
            "name": "Test Connection",
            "description": "Test if your Gmail OAuth token is valid",
            "http_method": "GET",
            "path_template": "/users/me/profile",
            "requires_confirmation": false,
            "request_schema": {"type": "object", "properties": {}}
        },
        {
            "operation_id": "gmail.send_email",
            "name": "Send Email",
            "description": "Send an email message",
            "http_method": "POST",
            "path_template": "/users/me/messages/send",
            "requires_confirmation": true,
            "request_schema": {
                "type": "object",
                "required": ["raw"],
                "properties": {
                    "raw": {"type": "string", "description": "Base64url encoded email message (RFC 2822 format)"}
                }
            }
        },
        {
            "operation_id": "gmail.list_messages",
            "name": "List Messages",
            "description": "List messages in the users mailbox",
            "http_method": "GET",
            "path_template": "/users/me/messages",
            "pagination_strategy": "token",
            "requires_confirmation": false,
            "request_schema": {
                "type": "object",
                "properties": {
                    "q": {"type": "string", "description": "Query string (same as Gmail search box)"},
                    "maxResults": {"type": "integer", "minimum": 1, "maximum": 500, "default": 100},
                    "pageToken": {"type": "string", "description": "Page token for pagination"},
                    "labelIds": {"type": "array", "items": {"type": "string"}, "description": "Filter by label IDs"}
                }
            }
        },
        {
            "operation_id": "gmail.get_message",
            "name": "Get Message",
            "description": "Get a specific email message",
            "http_method": "GET",
            "path_template": "/users/me/messages/{id}",
            "requires_confirmation": false,
            "request_schema": {
                "type": "object",
                "properties": {
                    "format": {"type": "string", "enum": ["minimal", "full", "raw", "metadata"], "default": "full"}
                }
            }
        },
        {
            "operation_id": "gmail.list_labels",
            "name": "List Labels",
            "description": "List all labels in the mailbox",
            "http_method": "GET",
            "path_template": "/users/me/labels",
            "requires_confirmation": false,
            "request_schema": {"type": "object", "properties": {}}
        }
    ]'::jsonb,
    '{
        "slug": "gmail",
        "category": "communication",
        "auth_type": "oauth2",
        "icon_url": "https://ssl.gstatic.com/ui/v1/icons/mail/rfr/logo_gmail_lockup_default_2x_r2.png",
        "documentation_url": "https://developers.google.com/gmail/api/reference/rest",
        "description": "Send emails, manage inbox, and organize messages",
        "auth_config": {
            "authorize_url": "https://accounts.google.com/o/oauth2/v2/auth",
            "token_url": "https://oauth2.googleapis.com/token",
            "scopes": [
                "https://www.googleapis.com/auth/gmail.send",
                "https://www.googleapis.com/auth/gmail.readonly",
                "https://www.googleapis.com/auth/gmail.compose",
                "https://www.googleapis.com/auth/gmail.labels"
            ],
            "access_type": "offline",
            "prompt": "consent",
            "setup_instructions": "Create OAuth credentials at https://console.cloud.google.com/"
        },
        "rate_limits": {"quota_units_per_user": 250, "quota_units_per_second": 25},
        "google_suite": true
    }'::jsonb
)
ON CONFLICT DO NOTHING;

-- ═══════════════════════════════════════════════════════════════════════════
-- 6. GOOGLE DRIVE (Google Suite - Storage)
-- ═══════════════════════════════════════════════════════════════════════════
INSERT INTO integrations (name, connector_type, endpoint_url, status, available_actions, metadata)
VALUES (
    'Google Drive',
    'storage',
    'https://www.googleapis.com/drive/v3',
    'disconnected',
    '[
        {
            "operation_id": "google_drive.test_connection",
            "name": "Test Connection",
            "description": "Test if your Google Drive OAuth token is valid",
            "http_method": "GET",
            "path_template": "/about?fields=user",
            "requires_confirmation": false,
            "request_schema": {"type": "object", "properties": {}}
        },
        {
            "operation_id": "google_drive.list_files",
            "name": "List Files",
            "description": "List files and folders in Google Drive",
            "http_method": "GET",
            "path_template": "/files",
            "pagination_strategy": "token",
            "requires_confirmation": false,
            "request_schema": {
                "type": "object",
                "properties": {
                    "q": {"type": "string", "description": "Query string for searching files"},
                    "pageSize": {"type": "integer", "minimum": 1, "maximum": 1000, "default": 100},
                    "pageToken": {"type": "string", "description": "Page token for pagination"},
                    "orderBy": {"type": "string", "description": "Sort order (e.g. modifiedTime desc)"},
                    "fields": {"type": "string", "description": "Fields to include in response"}
                }
            }
        },
        {
            "operation_id": "google_drive.create_folder",
            "name": "Create Folder",
            "description": "Create a new folder in Google Drive",
            "http_method": "POST",
            "path_template": "/files",
            "requires_confirmation": true,
            "request_schema": {
                "type": "object",
                "required": ["name"],
                "properties": {
                    "name": {"type": "string", "description": "Folder name"},
                    "mimeType": {"type": "string", "default": "application/vnd.google-apps.folder"},
                    "parents": {"type": "array", "items": {"type": "string"}, "description": "Parent folder IDs"}
                }
            }
        },
        {
            "operation_id": "google_drive.upload_file",
            "name": "Upload File",
            "description": "Upload a file to Google Drive (metadata only)",
            "http_method": "POST",
            "path_template": "/files",
            "requires_confirmation": true,
            "request_schema": {
                "type": "object",
                "required": ["name"],
                "properties": {
                    "name": {"type": "string", "description": "File name"},
                    "mimeType": {"type": "string", "description": "MIME type of the file"},
                    "parents": {"type": "array", "items": {"type": "string"}, "description": "Parent folder IDs"}
                }
            }
        },
        {
            "operation_id": "google_drive.get_file",
            "name": "Get File Metadata",
            "description": "Get metadata for a specific file",
            "http_method": "GET",
            "path_template": "/files/{fileId}",
            "requires_confirmation": false,
            "request_schema": {
                "type": "object",
                "required": ["fileId"],
                "properties": {
                    "fileId": {"type": "string", "description": "File ID"},
                    "fields": {"type": "string", "description": "Fields to return"}
                }
            }
        }
    ]'::jsonb,
    '{
        "slug": "google_drive",
        "category": "productivity",
        "auth_type": "oauth2",
        "icon_url": "https://ssl.gstatic.com/images/branding/product/2x/drive_2020q4_48dp.png",
        "documentation_url": "https://developers.google.com/drive/api/v3/reference",
        "description": "Store, sync, and share files in the cloud",
        "auth_config": {
            "authorize_url": "https://accounts.google.com/o/oauth2/v2/auth",
            "token_url": "https://oauth2.googleapis.com/token",
            "scopes": [
                "https://www.googleapis.com/auth/drive.file",
                "https://www.googleapis.com/auth/drive.readonly",
                "https://www.googleapis.com/auth/drive.metadata.readonly"
            ],
            "access_type": "offline",
            "prompt": "consent",
            "setup_instructions": "Create OAuth credentials at https://console.cloud.google.com/"
        },
        "rate_limits": {"queries_per_100_seconds": 1000, "queries_per_100_seconds_per_user": 100},
        "google_suite": true
    }'::jsonb
)
ON CONFLICT DO NOTHING;

-- ═══════════════════════════════════════════════════════════════════════════
-- 7. GOOGLE SHEETS (Google Suite - Productivity)
-- ═══════════════════════════════════════════════════════════════════════════
INSERT INTO integrations (name, connector_type, endpoint_url, status, available_actions, metadata)
VALUES (
    'Google Sheets',
    'custom',
    'https://sheets.googleapis.com/v4',
    'disconnected',
    '[
        {
            "operation_id": "google_sheets.test_connection",
            "name": "Test Connection",
            "description": "Test if your Google Sheets OAuth token is valid",
            "http_method": "GET",
            "path_template": "/spreadsheets?pageSize=1",
            "requires_confirmation": false,
            "request_schema": {"type": "object", "properties": {}}
        },
        {
            "operation_id": "google_sheets.get_spreadsheet",
            "name": "Get Spreadsheet",
            "description": "Get metadata about a spreadsheet",
            "http_method": "GET",
            "path_template": "/spreadsheets/{spreadsheetId}",
            "requires_confirmation": false,
            "request_schema": {
                "type": "object",
                "required": ["spreadsheetId"],
                "properties": {
                    "spreadsheetId": {"type": "string", "description": "Spreadsheet ID"},
                    "includeGridData": {"type": "boolean", "default": false}
                }
            }
        },
        {
            "operation_id": "google_sheets.get_values",
            "name": "Get Values",
            "description": "Read values from a range in a spreadsheet",
            "http_method": "GET",
            "path_template": "/spreadsheets/{spreadsheetId}/values/{range}",
            "requires_confirmation": false,
            "request_schema": {
                "type": "object",
                "required": ["spreadsheetId", "range"],
                "properties": {
                    "spreadsheetId": {"type": "string", "description": "Spreadsheet ID"},
                    "range": {"type": "string", "description": "A1 notation range (e.g. Sheet1!A1:D10)"},
                    "majorDimension": {"type": "string", "enum": ["ROWS", "COLUMNS"], "default": "ROWS"},
                    "valueRenderOption": {"type": "string", "enum": ["FORMATTED_VALUE", "UNFORMATTED_VALUE", "FORMULA"], "default": "FORMATTED_VALUE"}
                }
            }
        },
        {
            "operation_id": "google_sheets.update_values",
            "name": "Update Values",
            "description": "Write values to a range in a spreadsheet",
            "http_method": "PUT",
            "path_template": "/spreadsheets/{spreadsheetId}/values/{range}",
            "requires_confirmation": true,
            "request_schema": {
                "type": "object",
                "required": ["spreadsheetId", "range", "values"],
                "properties": {
                    "spreadsheetId": {"type": "string"},
                    "range": {"type": "string", "description": "A1 notation range"},
                    "valueInputOption": {"type": "string", "enum": ["RAW", "USER_ENTERED"], "default": "USER_ENTERED"},
                    "values": {"type": "array", "items": {"type": "array"}, "description": "2D array of cell values"}
                }
            }
        },
        {
            "operation_id": "google_sheets.append_values",
            "name": "Append Values",
            "description": "Append rows to the end of a table in a spreadsheet",
            "http_method": "POST",
            "path_template": "/spreadsheets/{spreadsheetId}/values/{range}:append",
            "requires_confirmation": true,
            "request_schema": {
                "type": "object",
                "required": ["spreadsheetId", "range", "values"],
                "properties": {
                    "spreadsheetId": {"type": "string"},
                    "range": {"type": "string", "description": "A1 notation range to append to"},
                    "valueInputOption": {"type": "string", "enum": ["RAW", "USER_ENTERED"], "default": "USER_ENTERED"},
                    "insertDataOption": {"type": "string", "enum": ["OVERWRITE", "INSERT_ROWS"], "default": "INSERT_ROWS"},
                    "values": {"type": "array", "items": {"type": "array"}, "description": "2D array of cell values to append"}
                }
            }
        }
    ]'::jsonb,
    '{
        "slug": "google_sheets",
        "category": "productivity",
        "auth_type": "oauth2",
        "icon_url": "https://upload.wikimedia.org/wikipedia/commons/3/30/Google_Sheets_logo_%282014-2020%29.svg",
        "documentation_url": "https://developers.google.com/sheets/api/reference/rest",
        "description": "Read and write Google Sheets spreadsheets",
        "auth_config": {
            "authorize_url": "https://accounts.google.com/o/oauth2/v2/auth",
            "token_url": "https://oauth2.googleapis.com/token",
            "scopes": ["https://www.googleapis.com/auth/spreadsheets"],
            "access_type": "offline",
            "prompt": "consent",
            "setup_instructions": "1. Go to Google Cloud Console\n2. Enable Google Sheets API\n3. Create OAuth 2.0 credentials"
        },
        "rate_limits": {"read_per_minute": 60, "write_per_minute": 60},
        "google_suite": true
    }'::jsonb
)
ON CONFLICT DO NOTHING;

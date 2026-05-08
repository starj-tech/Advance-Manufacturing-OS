-- Demo seed for `supabase db reset` during local dev.
-- Provisions: 1 tenant, role grants, 4 demo users (one per shell).

INSERT INTO tenants (id, slug, name, region) VALUES
    ('00000000-0000-0000-0000-000000000001', 'demo-factory', 'Demo Factory', 'us-east')
ON CONFLICT (id) DO NOTHING;

INSERT INTO roles (tenant_id, role_name, description) VALUES
    ('00000000-0000-0000-0000-000000000001', 'developer',  'Infrastructure & module ops'),
    ('00000000-0000-0000-0000-000000000001', 'executive',  'Leadership & finance visibility'),
    ('00000000-0000-0000-0000-000000000001', 'manager',    'Operations management'),
    ('00000000-0000-0000-0000-000000000001', 'employee',   'Shop floor operator')
ON CONFLICT DO NOTHING;

INSERT INTO permissions (tenant_id, role_name, scope, granted) VALUES
    ('00000000-0000-0000-0000-000000000001', 'developer', '*', TRUE),
    ('00000000-0000-0000-0000-000000000001', 'executive', 'kpi:read', TRUE),
    ('00000000-0000-0000-0000-000000000001', 'executive', 'twin:read', TRUE),
    ('00000000-0000-0000-0000-000000000001', 'executive', 'finance:read', TRUE),
    ('00000000-0000-0000-0000-000000000001', 'manager',   'work_orders:*', TRUE),
    ('00000000-0000-0000-0000-000000000001', 'manager',   'machines:read', TRUE),
    ('00000000-0000-0000-0000-000000000001', 'manager',   'inventory:read', TRUE),
    ('00000000-0000-0000-0000-000000000001', 'manager',   'maintenance:read', TRUE),
    ('00000000-0000-0000-0000-000000000001', 'employee',  'tasks:read', TRUE),
    ('00000000-0000-0000-0000-000000000001', 'employee',  'tasks:complete', TRUE),
    ('00000000-0000-0000-0000-000000000001', 'employee',  'sos:trigger', TRUE),
    ('00000000-0000-0000-0000-000000000001', 'employee',  'clock:write', TRUE)
ON CONFLICT DO NOTHING;

-- A few demo machines (operational data is plaintext — encrypted columns
-- like materials.name_ciphertext are populated by the client crypto layer).
INSERT INTO machines (id, tenant_id, code, name, status, hlc) VALUES
    ('11111111-1111-1111-1111-111111111101', '00000000-0000-0000-0000-000000000001', 'PRESS-01', 'Hydraulic press 250t', 'running', '0.0.seed'),
    ('11111111-1111-1111-1111-111111111102', '00000000-0000-0000-0000-000000000001', 'CNC-12',   'CNC mill A12',         'idle',    '0.0.seed'),
    ('11111111-1111-1111-1111-111111111103', '00000000-0000-0000-0000-000000000001', 'WELD-04',  'Robotic welder W4',    'fault',   '0.0.seed'),
    ('11111111-1111-1111-1111-111111111104', '00000000-0000-0000-0000-000000000001', 'PAINT-02', 'Powder coat line',     'maintenance', '0.0.seed')
ON CONFLICT DO NOTHING;

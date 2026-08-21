#!/usr/bin/env python3
"""
在 mars-test 数据库中创建/重置 Console demo 账号。

用法（在测试机或能连 PG 的机器上执行）：
  export DATABASE_URL="postgres://postgres:p@ssw0rd@192.168.10.16:5432/nerve"
  python3 scripts/reset-demo-user.py

默认账号/密码与 apps/console-react/src/pages/LoginPage.tsx 中的 demo 预填一致。
可通过环境变量覆盖：
  DEMO_USERNAME=lucongming_0108
  DEMO_PASSWORD=@Lucongming0108
"""
import os
import sys
import uuid

from argon2 import PasswordHasher

DB_URL = os.environ.get("DATABASE_URL", "postgres://postgres:p@ssw0rd@192.168.10.16:5432/nerve")
USERNAME = os.environ.get("DEMO_USERNAME", "lucongming_0108").strip()
PASSWORD = os.environ.get("DEMO_PASSWORD", "@Lucongming0108").strip()
ROLE = "admin"


def main() -> int:
    if not PASSWORD:
        print("DEMO_PASSWORD must not be empty", file=sys.stderr)
        return 1

    try:
        import psycopg2
    except ImportError:
        print("psycopg2 not installed; trying to install...", file=sys.stderr)
        os.system("{} -m pip install psycopg2-binary".format(sys.executable))
        import psycopg2

    ph = PasswordHasher()
    password_hash = ph.hash(PASSWORD)

    conn = psycopg2.connect(DB_URL)
    conn.autocommit = True
    cur = conn.cursor()

    cur.execute("SELECT waf_users_id FROM waf_users WHERE username = %s", (USERNAME,))
    row = cur.fetchone()
    if row:
        user_id = row[0]
        cur.execute(
            """
            UPDATE waf_users
               SET password_hash = %s,
                   role = %s,
                   enabled = true,
                   updated_at = now(),
                   updated_by_id_name = 'reset-demo-user'
             WHERE waf_users_id = %s
            """,
            (password_hash, ROLE, user_id),
        )
        print(f"updated user {USERNAME} ({user_id})")
    else:
        user_id = str(uuid.uuid4())
        cur.execute(
            """
            INSERT INTO waf_users
              (waf_users_id, username, role, password_hash, tenant_id, enabled,
               created_by_id_name, updated_by_id_name)
            VALUES (%s, %s, %s, %s, 'default', true, 'reset-demo-user', 'reset-demo-user')
            """,
            (user_id, USERNAME, ROLE, password_hash),
        )
        print(f"created user {USERNAME} ({user_id})")

    cur.close()
    conn.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())

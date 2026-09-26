#!/usr/bin/env python3
"""Fetch Stash's GraphQL introspection schema into schema.json.

Usage: fetch-schema.py [STASH_URL] [API_KEY]
       (defaults: http://localhost:9999, no key)
"""
import json
import pathlib
import sys
import urllib.request

QUERY = """
query IntrospectionQuery {
  __schema {
    queryType { name } mutationType { name } subscriptionType { name }
    types { ...FullType }
    directives { name description locations args { ...InputValue } }
  }
}
fragment FullType on __Type {
  kind name description
  fields(includeDeprecated: true) {
    name description args { ...InputValue } type { ...TypeRef } isDeprecated deprecationReason
  }
  inputFields { ...InputValue }
  interfaces { ...TypeRef }
  enumValues(includeDeprecated: true) { name description isDeprecated deprecationReason }
  possibleTypes { ...TypeRef }
}
fragment InputValue on __InputValue { name description type { ...TypeRef } defaultValue }
fragment TypeRef on __Type {
  kind name ofType { kind name ofType { kind name ofType { kind name ofType {
  kind name ofType { kind name ofType { kind name ofType { kind name } } } } } } }
}
"""

url = (sys.argv[1] if len(sys.argv) > 1 else "http://localhost:9999").rstrip("/") + "/graphql"
headers = {"Content-Type": "application/json"}
if len(sys.argv) > 2:
    headers["ApiKey"] = sys.argv[2]

req = urllib.request.Request(url, data=json.dumps({"query": QUERY}).encode(), headers=headers)
data = json.load(urllib.request.urlopen(req, timeout=30))
if "errors" in data:
    sys.exit(f"introspection failed: {data['errors']}")

out = pathlib.Path(__file__).with_name("schema.json")
out.write_text(json.dumps(data, indent=1) + "\n")
print(f"wrote {out} ({len(data['data']['__schema']['types'])} types)")

#!/usr/bin/env bash
# mars-test 发布：从 baseline 副本中剔除指定规则（不修改 Nerve-antibody 源码树）。

pack_baseline_filter_for_test() {
  local baseline_dir="$1"
  shift
  local -a exclude_ids=("$@")
  if [[ ! -d "$baseline_dir/rules" ]]; then
    echo "pack-baseline-test-filter: missing rules dir: $baseline_dir/rules" >&2
    return 1
  fi

  local id f removed=0
  for id in "${exclude_ids[@]}"; do
    while IFS= read -r -d '' f; do
      rm -f "$f"
      removed=$((removed + 1))
      echo ">> test pack: removed rule file $f"
    done < <(find "$baseline_dir/rules" -type f \( -name "${id}.yaml" -o -name "${id}.yml" \) -print0 2>/dev/null)
  done

  if [[ $removed -eq 0 ]]; then
    echo ">> test pack: no rule files removed (ids: ${exclude_ids[*]})" >&2
  fi

  # 样本库仅用于 bench；剔除对应条目避免 mars-test 门禁误报。
  local samples="$baseline_dir/samples/05-rfi-ssrf.yaml"
  if [[ -f "$samples" ]] && command -v python3 >/dev/null 2>&1; then
    python3 - "$samples" "${exclude_ids[@]}" <<'PY'
import sys
from pathlib import Path

path = Path(sys.argv[1])
drop = set(sys.argv[2:])
text = path.read_text(encoding="utf-8")
blocks = text.split("\n- id:")
out = [blocks[0]] if blocks else [""]
removed = 0
for block in blocks[1:]:
    chunk = "- id:" + block
    skip = False
    for rid in drop:
        if f"rule_id: {rid}" in chunk:
            skip = True
            removed += 1
            break
    if not skip:
        out.append(block)
path.write_text("\n".join(out), encoding="utf-8")
print(f">> test pack: pruned {removed} sample(s) from {path.name}")
PY
  fi

  return 0
}

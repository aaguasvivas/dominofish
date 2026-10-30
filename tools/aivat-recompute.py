#!/usr/bin/env python3
# Recomputes the cross-fitted AIVAT correction of stats::crossfit
# (crates/dominofish/src/stats.rs) from a match result file's pair and aivat
# lines, with the terms per round (the harness before the fix of Q12,
# docs/PLAN.md D19) or as per-pair totals (the harness since), and prints the
# mean, standard error, and t of corrected minus raw for each. Test T7's
# criterion (d) uses it (scripts/verify/M5-03.sh): it must reproduce T6's
# rows per round and T7's rows with totals, and fail T7's game rows per
# round. Usage: python3 -I tools/aivat-recompute.py <result file>...
import math, sys
def fit(rows):
    n = len(rows); mean_y = sum(r[0] for r in rows) / max(n, 1)
    mx = [sum(r[1][c] for r in rows) / max(n, 1) for c in range(3)]
    live = [c for c in range(3) if any(abs(r[1][c] - mx[c]) > 1e-9 for r in rows)]
    k = len(live); b = [0.0, 0.0, 0.0]
    if k == 0 or n < k + 2: return b
    m = [[0.0] * (k + 1) for _ in range(k)]
    for y, x in rows:
        for i, ci in enumerate(live):
            xi = x[ci] - mx[ci]
            for j, cj in enumerate(live): m[i][j] += xi * (x[cj] - mx[cj])
            m[i][k] += xi * (y - mean_y)
    for i in range(k):
        p = max(range(i, k), key=lambda a: abs(m[a][i])); m[i], m[p] = m[p], m[i]
        if abs(m[i][i]) < 1e-9: return [0.0, 0.0, 0.0]
        piv = m[i][:]
        for r in range(k):
            if r != i:
                f = m[r][i] / piv[i]
                for c in range(i, k + 1): m[r][c] -= f * piv[c]
    for i, ci in enumerate(live): b[ci] = m[i][k] / m[i][i]
    return b
def crossfit(rows):
    folds = [[r for i, r in enumerate(rows) if i % 2 == f] for f in (0, 1)]
    b = [fit(folds[1]), fit(folds[0])]
    return [y - sum(b[i % 2][j] * x[j] for j in range(3)) for i, (y, x) in enumerate(rows)], b
def load(path, per_round):
    lines = open(path).read().splitlines()
    pairs = {}; terms = {}
    for l in lines:
        w = l.split()
        if l.startswith("pair "):
            r = w[w.index("rounds") + 1].split(","); pairs[int(w[1])] = (int(w[w.index("margin") + 1]), int(r[0]) + int(r[1]))
        elif l.startswith("aivat "):
            terms[int(w[1])] = [float(w[3]), float(w[5]), float(w[7])]
    rows = []
    for i in sorted(terms):
        margin, rounds = pairs[i]
        x = [t / rounds for t in terms[i]] if per_round else terms[i]
        rows.append((margin / rounds, x))
    return rows
def t_of(path, per_round):
    rows = load(path, per_round); corr, b = crossfit(rows)
    d = [c - r[0] for c, r in zip(corr, rows)]; n = len(d); m = sum(d) / n
    sd = math.sqrt(sum((x - m) ** 2 for x in d) / (n - 1)); se = sd / math.sqrt(n)
    return m, se, m / se, b
if __name__ == "__main__":
    for p in sys.argv[1:]:
        print(p.split("/")[-1], *("%s: d %+.4f se %.4f t %+.3f" % (lab, *t_of(p, pr)[:3]) for lab, pr in (("per round", True), ("totals", False))), sep="  |  ")

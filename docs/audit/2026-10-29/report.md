# Ship audit, 2026-10-29: the web board and the README

Scope: the web board (web/, built by scripts/build-web.sh into web/dist/index.html), its hosting, and the README. Owner request (PLAN.md decision 9): "Can you ensure it is high quality, polished, professional, ready to go, we should host it too". Builds checked: HEAD aef5028, served from web/dist on a local server and opened in the browser pane. Nothing is deployed yet.

## Verdict

The board is ready to ship: every fix below is verified in the built page at the seven audit sizes in English and Spanish. Hosting waits for the Worker name (Q10 in docs/PLAN.md).

## Fix list

Each item names the build it describes. "Verified" means checked in the built page in the browser pane, not on a live host.

1. **fix, verified (HEAD).** False claim: the hint said it came "From the rule bot, the strongest engine here". The hint and the Expert bot are now DominoFish (PIMC, 32 worlds; R5 measured PIMC at 32 worlds above the rule bot: 6.01 [4.28, 7.74] points per round in 1v1, 12.71 [10.25, 15.17] in 2v2). The README board text and docs/assets/board.png (a new 800x754 capture) show the new hint. grep "From the rule bot" over web/src, web/dist, README.md: 0 hits.
2. **fix, verified (HEAD).** The hint says each sample is solved exactly, but a 1v1 solve can reach the 2,000,000-node cap. The engine reply now carries the capped count, and the hint then shows "Approximate: a solve reached its node cap."
3. **fix, verified (HEAD).** Layout shift at load: the engine's first view filled the empty score bar and table and moved the table, hand, and panel down. Cumulative layout shift 0.1925 at about 545 ms (375x667), then no shift entry after the fix: the layout stays hidden behind loading dots until the first view or the engine error.
4. **fix, verified (HEAD).** The Google Fonts stylesheet was render-blocking, and the inline script that starts the engine waited for it. Now non-blocking (renderBlockingStatus "non-blocking"); the fonts still apply.
5. **fix, verified (HEAD).** Brand name truncated to "DominoF..." at 320 px: it needs 95 px and had 88 px. Logo 36 to 32 px and two gaps 4 and 2 px smaller give it 98 px; clipped text: none.
6. **fix, verified (HEAD).** Hint row at 320 px: the sampling error broke into two lines ("±" above "7.3"). Now one line (18.75 px high in each row).
7. **fix, verified (HEAD).** Move list at 320 px: "Compañero" (73 px) overran its 58 px column into the tile. The column is 80 px at every width; name overflow 0.
8. **fix, verified (HEAD).** Short landscape: the table's row template had four sizes for three rows, so at 667x375 a 100 px row held one 30 px name plate; at 568x320 the line of play (259 to 307 px) started under the docked hand (top 266 px). Now the plates share a row at 568x320: line 170 to 218 px, "You" plate 232 to 259 px, hand 266 px; at 667x375 the hand sits below the table (280 px against 272 px).
9. **fix, verified (HEAD).** The phone rule that makes the header's New game button icon-only (44 px) also squeezed each dialog's primary button: "Deal"/"Repartir" showed as "Reparti" (89 px of text in 44 px). The rule now applies to the header only.
10. **fix, verified (HEAD).** Round dialog at 320 px: its two buttons needed 262 px in 240 px and ran 4 px past the dialog's left edge. Below 640 px the buttons now fill the row and stack when they do not fit (primary at the bottom).
11. **fix, verified (HEAD).** New game dialog at 320x568: Cancel and Deal sat at 519 to 563 px, below the dialog's bottom edge (545 px). The dialog actions are now a sticky footer (479 to 523 px), also in the 568x320 landscape dialog.
12. **fix, verified (HEAD).** Bot names truncated in the dialog's selects at 320 px ("Experto (DominoFish, el mot..."; "Difícil (cuenta fichas, lee los pases)" is 241 px in a 240 px select). Shorter names: Expert (DominoFish), Hard (reads passes), Easy (heaviest tile); Spanish Experto (DominoFish), Difícil (lee los pases), Fácil (bota gorda). Dialogs use 16 px side padding at 380 px and below, so the select is 252 px; the longest names, "Medium (most accompanied)" (199 px) and "Medio (la más acompañada)" (193 px), show whole on screen at 320 px.
13. **fix, verified (HEAD).** In 1v1 the dialog still showed the Partner select: `.field { display: flex }` overrode the `hidden` attribute. A `[hidden]` rule fixes it.
14. **fix, verified (HEAD).** Accessible names: at phone width the header's New game button had none (its label is display: none), and the deal number field was announced by its value. Both now have translated labels.
15. **fix, verified (HEAD).** Copy, both languages: sentences with the human seat as subject used the third person ("Tú tiene 29", "You wins the game", "Us win the game", "Jugadas de Tú", "You | passes"). They now use the second person ("Te llevas la ronda", "You win the game", "We win the game", "Tus jugadas", "You | pass"). The hint's margin sentence repeated the best value, which wrapped alone onto a second line; it now explains what the values mean. 104 keys in each language, none unused.
16. **fix, verified (HEAD).** Page head: a noscript notice in both languages, theme-color and Open Graph tags; the claude.ai artifact meta tag moved into the artifact copy only.
17. **owner decision (open).** Hosting: the Worker name (Q10) and the deploy method. Live checks after the deploy: the page's build string, a missing path returning 404 with no board page, cache headers on the HTML, and the hint and analysis on a phone.

Sweeps: em dash and en dash over web/src, web/dist, README.md, docs/, crates/, scripts/, tools/: 0 hits (a known-bad sample was caught). Overflow probe at 320x568, 568x320, 375x667, 375x548, 667x375, 430x932, and 1440x900: 0 elements past the viewport and no clipped text (a 2000 px known-bad element was caught). Light and dark themes checked at 375x667 and 1440x900. Console: no errors.

Web performance (reduced audit, browser pane, local server, one uncontrolled load each, so not lab-grade): DOMContentLoaded 33 to 37 ms, no long tasks, layout shift 0 after fix 3. Not measured: LCP and FCP (no paint entries in the pane), INP, TBT, and transfer size and compression on the real host.

## What's left for you

1. Answer Q10: the Worker name (my recommendation: dominofish, so the board is at dominofish.adelsonaguasvivas.workers.dev), and the deploy method.
2. After the deploy, open the address on your phone, play a round, ask for a hint, and turn on the exact analysis.
3. Later, add your domain: Cloudflare dashboard, Workers & Pages, the Worker, Settings, Domains & Routes, Add, Custom domain.
4. Answer Q11 in docs/PLAN.md (T5's power check failed; M5-03 does not pass).
5. Optional: the private claude.ai artifact of the board (https://claude.ai/artifact/96zYMxRAJySo2pxhUUjMNW) shows an older version. Delete it, or keep it as a private copy.

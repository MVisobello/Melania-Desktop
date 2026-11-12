# Melania Fluent: inventario esatto di Flat Remix Yellow

Il generatore `tools/build_melania_exact_flat_remix.py` conserva **percorsi,
nomi di file, collegamenti simbolici e struttura delle cartelle** dell'archivio
originale `04-Flat-Remix-Yellow-20251119.tar.xz` fornito per il progetto,
ma disegna tutti gli SVG usando esclusivamente i moduli Melania già presenti
nel repository. Il contenuto grafico degli SVG Flat Remix non viene letto,
copiato o ricolorato.

Tre pacchetti generati:

- `Melania-Light` — per interfacce chiare.
- `Melania-Dark` — per interfacce scure.
- `Melania-Light-darkPanel` — interfaccia chiara con barra scura.

## Uso

```bash
python3 tools/build_melania_exact_flat_remix.py \
  --reference ~/Scaricati/04-Flat-Remix-Yellow-20251119.tar.xz \
  --output dist/melania-exact-icons
python3 tests/test_exact_flat_remix_inventory.py
```

L'archivio Flat Remix originale non è pubblicato nel repository.
La sua struttura di collegamenti crea dipendenze tra le cartelle:
**installare sempre insieme tutti e tre i pacchetti**.

### Perché serve un'ulteriore revisione grafica

La compatibilità dei nomi garantisce la risoluzione dei percorsi,
ma non che ogni applicazione abbia già un pittogramma unico e
approvato. Le icone non coperte dai disegni semantici dedicati
devono essere rifinite individualmente. Il report di build distingue
le provenienze dei disegni e verifica i collegamenti.

Questo generatore è distinto da `build_melania_fluent_themes.py`:
quello genera la copertura ampliata di Melania, mentre questo
rispetta **esattamente la struttura del riferimento** senza aggiunte.

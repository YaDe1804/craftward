# SeQu-Check Craftward v1

**Datum:** 15.11.2026
**Geprüfter Stand:** `main`, Commit `187666a` (AP8), identisch mit der hochgeladenen ZIP-Datei (einzige Abweichung: `LICENSE` lokal mit CRLF-Zeilenenden, auf GitHub mit LF)
**Umfang:** alle fünf Crates (`craftward-core`, `-cli`, `-net`, `-store`, `-app`), CI-Workflow, `deny.toml`, Abhängigkeitsdeklarationen. Rund 14.600 Zeilen Quellcode ohne Tests.
**Maßstab:** Enterprise-Grade, Best Practices, Clean Code, kein Boilerplate.

## Ergebnis: CONDITIONAL PASS

Keine kritischen oder hohen Befunde. Die Sicherheitsgrundlagen sind solide. Es gibt aber vier mittlere Befunde zur Datenintegrität und Korrektheit und drei mittlere Befunde zur Codequalität, die vor der Desktop-App behoben werden sollten.

| Schwere | Anzahl |
| --- | --- |
| Kritisch | 0 |
| Hoch | 0 |
| Mittel | 8 |
| Niedrig | 7 |
| Hinweis | 2 |

## Nicht geprüft

| Prüfung | Grund | Was nötig wäre |
| --- | --- | --- |
| Bauen und Tests ausführen | Die festgelegte Toolchain Rust 1.100.0 ist in der Prüfumgebung nicht ladbar; mit Rust 1.97 kompiliert der Code nicht (`map_or_default`) | Lauf auf deinem Rechner oder in der CI (dort laut PR #9 grün auf allen drei Systemen) |
| `cargo deny` / Schwachstellenabgleich der Abhängigkeiten | wie oben | CI-Lauf (grün laut PR #9) |
| Dynamische Prüfung, Fuzzing | kein lauffähiger Build | Fuzzing-Ziele (siehe Roadmap Phase 2, Aufgabe t11) |
| Laufzeitverhalten unter Windows und macOS | nur statische Prüfung | Handtest bzw. CI |

## Teilweise geprüft

Vollständig gelesen: `craftward-net`, `craftward-store`, `workspace.rs`, `paths.rs`, `cache.rs`, `http.rs`, `osv.rs`, `lockfile.rs`, `target_sdk.rs`, CI, `deny.toml`, Cargo-Manifeste.
Gezielt gelesen (Struktur, Schreibwege, Vergleichslogik, Escaping): `eol.rs`, `roadmap.rs`, `edit.rs`, `location.rs`, CLI.
Nur stichprobenartig: `lifecycle.rs`, `radar.rs`, `load.rs`, `deadlines.rs`, `i18n.rs`. Befunde in diesen Dateien sind daher nicht ausgeschlossen.

## Was gut ist

- `unsafe` ist im ganzen Workspace verboten (`forbid`).
- Alle SQL-Anweisungen sind parametrisiert. Dynamisch zusammengesetzt wird nur die konstante Spaltenliste.
- Netzwerk nur über HTTPS (doppelt abgesichert: `https_only` im Agent und eigene Prüfung), Zeitlimits auf allen Phasen, Größenlimit pro Antwort, Zertifikate aus dem Betriebssystem.
- HTML-Ausgabe der Roadmap vollständig maskiert, Links nur mit vorher validierten http(s)-Adressen.
- Validierung sammelt alle Fehler mit genauer Stelle; Bearbeitungen werden vor dem Schreiben validiert.
- Datenbank-Migrationen laufen je in einer Transaktion zusammen mit `user_version`.
- CI mit Actions auf Commit-SHAs, minimalen Rechten, `cargo deny` und Tests auf drei Systemen.
- Saubere Trennung: Netzwerk nur in `craftward-net`, SQLite nur in `craftward-store`.

## Befunde

### SQ-01 · Mittel · Atomares Schreiben ohne Dauerhaftigkeit

**Ort:** `craftward-core/src/workspace.rs`, `write_atomic` und `temporary_path`
**Befund:** Die Datei wird mit `fs::write` geschrieben und sofort umbenannt. Es fehlt `sync_all()` auf der Datei vor dem Umbenennen und unter Unix ein `fsync` auf das Verzeichnis danach. Je nach Dateisystem kann nach einem Stromausfall oder Absturz eine leere oder abgeschnittene Datei unter dem endgültigen Namen stehen. Betroffen sind `craftward.json` (Quelle der Wahrheit für die Planung), `workspace.json` und die Caches.
Zusätzlich wird die temporäre Datei mit `fs::write` angelegt, also ohne `create_new`. Ein vorhandener Pfad gleichen Namens würde überschrieben bzw. einem symbolischen Link gefolgt. Der Name ist vorhersagbar (PID plus Nanosekunden).
**Belegt:** statisch.
**Empfehlung:** `OpenOptions::new().write(true).create_new(true)`, `write_all`, `sync_all`, `rename`, unter Unix danach das Elternverzeichnis öffnen und `sync_all`. Beim Fehler die temporäre Datei entfernen. Eine einzige Implementierung für alle Schreibwege.

### SQ-02 · Mittel · Sicherungskopie der Datenbank ignoriert die WAL-Datei

**Ort:** `craftward-store/src/lib.rs`, `copy_backup`
**Befund:** Die Datenbank läuft im WAL-Modus. Die Sicherung vor einer Migration ist ein `fs::copy` der Hauptdatei. Bestätigte Transaktionen, die noch in `craftward.db-wal` liegen (nach einem Absturz oder während eine zweite Instanz offen ist), fehlen in der Kopie. Die Sicherung ist dann unvollständig oder inkonsistent, und das fällt erst auf, wenn man sie braucht.
**Belegt:** statisch.
**Empfehlung:** Sicherung über die Online-Backup-Schnittstelle von SQLite (rusqlite-Feature `backup`) oder `VACUUM INTO`. Die Sicherung danach probeweise öffnen und `PRAGMA integrity_check` ausführen.

### SQ-03 · Mittel · Verzögerte Transaktionen führen bei parallelen Läufen zu „database is locked"

**Ort:** `craftward-store/src/lib.rs`, `in_transaction` (`self.conn.transaction()`)
**Befund:** `Connection::transaction()` startet eine verzögerte Transaktion (`DEFERRED`). Der Lebenszyklus liest erst und schreibt dann. Laufen zwei Radar-Läufe gleichzeitig (z. B. Terminal und CI auf demselben Rechner), kann das Hochstufen von Lese- auf Schreibsperre sofort mit `SQLITE_BUSY` scheitern. Der `busy_timeout` greift in diesem Fall nicht, weil SQLite eine Verklemmung vermeidet. Keine Datenbeschädigung, aber ein abgebrochener Lauf.
**Belegt:** statisch (dokumentiertes SQLite-Verhalten).
**Empfehlung:** `transaction_with_behavior(TransactionBehavior::Immediate)` für alle schreibenden Sitzungen.

### SQ-04 · Mittel · targetSdk-Erkennung kann einen zu niedrigen Wert verdecken

**Ort:** `craftward-core/src/target_sdk.rs`, `detect`, `strip_gradle`
**Befund:** Bei mehreren Treffern gilt der höchste Wert. Treffer werden aber auch in Zeichenketten (`strip_gradle` kopiert String-Inhalte unverändert) und in anderen Blöcken gefunden, etwa `testOptions { targetSdk = … }` oder `lint { targetSdk = … }`. Ein höherer Wert an solch einer Stelle verdeckt den tatsächlichen, zu niedrigen targetSdk der App. Das Radar meldet dann keine Frist, obwohl eine abgelaufen ist. Für ein Prüfwerkzeug ist das die gefährliche Richtung (fail-open).
Die Regel „der höchste gilt" stammt aus meiner Arbeitsanweisung für AP8. Der Fehler liegt also in der Vorgabe, nicht in Cursors Umsetzung.
**Belegt:** statisch.
**Empfehlung:** Zeichenketten-Inhalte beim Vorverarbeiten leeren; Zuweisungen innerhalb von `testOptions`, `lint` und `managedDevices` ignorieren; bei abweichenden Werten den niedrigsten verwenden (fail-safe) und alle Fundstellen als Hinweis ausgeben.

### SQ-05 · Mittel · Doppelter Code in mehreren Modulen

**Orte:**
- acht identische Validierungs-Helfer in `load.rs` und `workspace.rs` (`reject_unknown`, `expect_object`, `expect_array`, `expect_string`, `join`, `push`, `newer_schema_version`, `u64_number`)
- Prozent-Kodierung viermal: `cache.rs::encode`, `osv.rs::encode_path`, `eol.rs::encode_path`, `eol.rs::encode_file`
- Cache-Lesen und -Schreiben zweimal: `cache.rs` und eigene Funktionen in `eol.rs` (`read_cached`, `write_cached`, `use_cache`)
- Pfadauflösung mehrfach: `paths::join_stored`, `target_sdk::resolve`, `cli/main.rs::platform_path`
- Antwortprüfung zweimal: `osv.rs::accepted_body` und `eol.rs::get_json`

**Befund:** Jede Kopie muss bei Änderungen einzeln gepflegt werden; Abweichungen schleichen sich ein (die Kodierungen unterscheiden sich bereits im Detail).
**Empfehlung:** gemeinsame Module für JSON-Validierung, Prozent-Kodierung, einen generischen JSON-Cache mit Gültigkeitsdauer, Pfadauflösung und HTTP-Antwortprüfung.

### SQ-06 · Mittel · Domänenwerte als Zeichenketten statt Typen

**Orte:** `Advisory.kind`, `Advisory.severity`, `StoredFinding.kind`/`severity`, Ereignisarten in `insert_event(kind: &str)`, `ParsedLock.kind`, `HttpRequest.method`
**Befund:** Zustand (`FindingState`) ist ein Enum, Art und Schwere sind Zeichenketten. Die einzige harte Prüfung der erlaubten Werte steckt in den `CHECK`-Listen der Datenbank. Folge: Jede neue Art erzwang einen Neuaufbau von Tabellen (Schema 3, 4 und 5). Bei `HttpRequest.method` fällt jeder unbekannte Wert stillschweigend auf `GET` zurück (`_ =>`).
**Empfehlung:** Enums `FindingKind`, `Severity`, `EventKind`, `Method`, `LockKind` im Kern mit `as_str`/`FromStr` und serde-Ableitung. In einer abschließenden Migration die `CHECK`-Listen für Art und Ereignis entfernen; die Prüfung übernimmt der Typ beim Lesen (unbekannter Wert = klarer Fehler).

### SQ-07 · Mittel · Handgeschriebenes JSON statt typisierter Strukturen

**Orte:** `osv.rs` (Antworten per `Value`-Traversierung, Anfrage per `Map` zusammengebaut), `eol.rs`, `deadlines.rs`, `workspace.rs::format_workspace` (JSON als Formatstring)
**Befund:** Viel Boilerplate, schwer lesbar, Tippfehler in Feldnamen fallen erst zur Laufzeit auf. Die handgeschriebene Validierung in `load.rs` und `workspace.rs` ist dagegen begründet, weil sie alle Fehler mit Pfad sammelt.
**Empfehlung:** `#[derive(Deserialize)]`-Strukturen für die Antworten von OSV und endoflife.date sowie die Fristen-Datei, `#[derive(Serialize)]` für die Batch-Anfrage und die Workspace-Datei.

### SQ-08 · Mittel · Handgeschriebene Argumentverarbeitung in der CLI

**Orte:** `craftward-cli/src/main.rs`, `editing.rs`, `stack.rs`, `radar.rs`, `findings.rs`, `deadlines.rs`
**Befund:** Rund 110 Optionen werden über etwa 1.900 Zeilen von Hand geparst, mit wiederkehrenden Mustern (`assign`, `parse_one_word`, `unexpected`, eigene Hilfetexte). Das ist Boilerplate und wächst mit jedem Befehl weiter. Die Vorgabe „Argumente nur mit der Standardbibliothek" stammt aus meinen Arbeitsanweisungen.
**Empfehlung:** Entscheidung nötig: entweder ein kleiner, deklarativer Parser als eigenes Modul (bleibt abhängigkeitsfrei) oder `clap` (sehr verbreitet, MIT/Apache-2.0, nimmt Hilfe und Fehlermeldungen ab, bringt aber Pakete mit).

### SQ-09 · Niedrig · Erkennung der crates.io-Quelle per Teilstring

**Ort:** `lockfile.rs::is_crates_io`
**Befund:** `source.contains("…crates.io-index")` akzeptiert auch fremde Registries, deren URL diesen Text enthält. Folge: falsche OSV-Abfragen (falsch-positive oder falsch-negative Treffer).
**Empfehlung:** exakter Vergleich mit `registry+https://github.com/rust-lang/crates.io-index` und `sparse+https://index.crates.io/`.

### SQ-10 · Niedrig · Cargo.lock-Leser zerlegt Nicht-ASCII-Zeichen

**Ort:** `lockfile.rs::parse_string`
**Befund:** Die Zeichenkette wird byteweise in `char` umgewandelt. Mehrbyte-UTF-8 wird so zu falschen Zeichen. Auf crates.io sind Namen ASCII, daher praktisch ohne Auswirkung, aber fachlich falsch.
**Empfehlung:** über `chars()` iterieren.

### SQ-11 · Niedrig · Cache-Dateinamen mit Punkt-Segmenten

**Ort:** `cache.rs::encode` (und Kopien)
**Befund:** `.` und `..` bleiben unkodiert. Ein Paketname `.` oder `..` aus einer `package-lock.json` fremder Herkunft führt zu Kollisionen im Cache-Verzeichnis. Ein Ausbruch aus dem Cache-Ordner ist nicht möglich, weil je Ebene nur ein Segment steuerbar ist und Dateinamen auf `.json` enden (statisch belegt).
**Empfehlung:** führende Punkte kodieren oder Dateinamen aus einem Hash bilden.

### SQ-12 · Niedrig · Relative XDG-Pfade werden akzeptiert

**Ort:** `location.rs`, `default_workspace_file`, `default_cache_dir`, `default_data_dir`
**Befund:** Die XDG-Spezifikation verlangt, relative Werte von `XDG_CONFIG_HOME`, `XDG_CACHE_HOME` und `XDG_DATA_HOME` zu ignorieren. Craftward übernimmt sie und schreibt dann relativ zum aktuellen Ordner.
**Empfehlung:** nur absolute Werte übernehmen, sonst auf `~/.config` usw. zurückfallen.

### SQ-13 · Niedrig · Abhängigkeiten exakt festgenagelt, CI ohne `--locked`

**Orte:** `Cargo.toml` (Workspace), `craftward-net/Cargo.toml`, `craftward-store/Cargo.toml`, `.github/workflows/ci.yml`
**Befund:** Alle Abhängigkeiten stehen mit `=` im Manifest. Bei einer Anwendung pinnt bereits `Cargo.lock`; das `=` verhindert zusätzlich, dass `cargo update` kompatible Sicherheitskorrekturen übernimmt. Gleichzeitig laufen Clippy und Tests in der CI ohne `--locked` und würden eine abweichende Lockdatei stillschweigend anpassen.
**Empfehlung:** normale Versionsangaben im Manifest, `--locked` in allen CI-Schritten. Automatische Update-PRs (Dependabot) für Cargo und GitHub Actions.

### SQ-14 · Niedrig · Unnötiger und verschleiernder Code

- `lockfile.rs::finish_package`: ungenutzter Parameter `_next_line`
- `roadmap.rs::escape_html`: öffentliche Hülle um die private `escape` ohne Mehrwert
- `craftward-store`: `busy_timeout` doppelt gesetzt (API und Pragma), dazu `i64::try_from(…).unwrap_or(5000)` für eine Konstante
- `workspace.rs::format_project`, `cache.rs::write_json`: `unwrap_or_else(|_| "{}")` verschluckt einen Serialisierungsfehler statt ihn weiterzugeben
- `craftward-store::open_in_memory`: Doku sagt „schema version 1", migriert aber auf die aktuelle Version

### SQ-15 · Niedrig · Filterung der Treffer im Speicher statt in SQL

**Ort:** `craftward-store/src/lib.rs::list_findings`
**Befund:** Es werden alle Treffer geladen und danach in Rust gefiltert. Bei vielen Projekten und langem Verlauf unnötig langsam.
**Empfehlung:** Zustand und Projekte in die `WHERE`-Klausel.

### SQ-H1 · Hinweis · Schwere bei RustSec-Meldungen immer „unknown"

Die Schwere stammt nur aus `database_specific.severity`. RustSec liefert dort nichts, sondern einen CVSS-Vektor im Feld `severity`. Craftward zeigt Rust-Lücken deshalb ohne Schwere. Eine eigene CVSS-Berechnung wäre möglich; Lockward enthält bereits eine, und da du Rechteinhaber von Lockward bist, kannst du den Code auch unter Apache-2.0 in Craftward verwenden.

### SQ-H2 · Hinweis · Vertrauen in fremde Projektdateien

`source_dir` darf ein beliebiger absoluter Pfad sein. Eine `craftward.json` aus einem fremden Repo kann das Radar also Lockdateien an anderer Stelle lesen und die Paketnamen an OSV senden lassen. Es wird kein Code ausgeführt und nichts anderes übertragen. In der Doku sollte stehen, dass Projektdateien vertrauenswürdige Eingaben sind.

## Behebungspakete

| Paket | Befunde | Inhalt |
| --- | --- | --- |
| F1 Datenintegrität | SQ-01, SQ-02, SQ-03 | dauerhaftes atomares Schreiben, Sicherung über Backup-API, sofortige Schreibtransaktionen |
| F2 Typen und Datenbank | SQ-06, SQ-15, Teile von SQ-14 | Enums für Domänenwerte, abschließende Migration ohne Werte-Listen, Filter in SQL |
| F3 Duplikate und typisiertes JSON | SQ-05, SQ-07, Rest von SQ-14 | gemeinsame Module, serde-Strukturen |
| F4 Korrektheit Radar | SQ-04, SQ-09, SQ-10, SQ-11, SQ-12 | targetSdk fail-safe, exakte Quellen, UTF-8, Cache-Namen, XDG |
| F5 Build und Lieferkette | SQ-13 | Versionsangaben, `--locked`, Dependabot |
| offen | SQ-08 | Entscheidung zur Argumentverarbeitung |
| später | SQ-H1, SQ-H2 | CVSS-Berechnung, Doku zum Vertrauensmodell |

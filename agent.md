# Fallen Derleyicisi — Geliştirme Özeti

Bu dosya, projeyi devralacak geliştiricinin/agent'ın fikri ve mevcut durumu hızlıca
anlaması için yazıldı. Yeni bir oturumda önce bunu oku, sonra `src/` altına bak.

---

## 1. Proje fikri

Fallen, C benzeri sözdizimine sahip fakat şu ek avantajları hedefleyen bir dildir:

- **C benzeri sözdizimi** (değişkenler, pointer'lar, çok boyutlu diziler,
  fonksiyonlar/prototipler, kontrol akışı, `switch/case`, `#include`, `#define`).
- **Bellek koruması** hedefi: doğrudan dizi erişimlerinde çalışma zamanı sınır
  kontrolü, otomatik sıfır başlatma, private/protected erişim denetimi, fork/exec
  güvenliği.
- **OOP**: `class`, `private/protected/public`, alanlar, metotlar, constructor, `this`,
  `.` ve `->`, otomatik constructor çağrısı.
- **Derleme anında statik bellek analizi**: sınıf boyutları, alan ofsetleri,
  her fonksiyonun stack çerçevesi ve toplam statik stack rezervi hesaplanır.
- **Doğrudan makine kodu**: `.flen` → x86-64 GNU assembly → `as` → `ld`.
  Araya C kodu girmez. `cc` yalnızca `ld` bulunamazsa yedek bağlayıcıdır.
- **Çok hızlı derleme**: shell örneğinde derleyici aşamaları ~0.5 ms,
  `as + ld` dahil toplam ~20 ms.
- Dosya uzantısı: **`.flen`**.

Referans program: `examples/shell.flen` (minimalist, sınıf tabanlı kabuk).
Bu programın derlenip çalışması projenin ana kabul testidir.

---

## 2. Mevcut durum

Çalışan sürüm tamamlandı (v0.3). `examples/shell.flen` derleniyor,
çalışıyor ve `tests/run.sh` içindeki tüm testler geçiyor.

### Derleme ve kullanım

```bash
cd /home/berat/fallen
cargo build --release
./tests/run.sh

target/release/fallen build dosya.flen -o cikti
target/release/fallen build dosya.flen -o cikti --stats
target/release/fallen build dosya.flen -S          # sadece .s üret
target/release/fallen build dosya.flen -o cikti --keep-asm
target/release/fallen run dosya.flen               # derle + çalıştır
target/release/fallen check dosya.flen             # kod üretmeden semantik kontrol
target/release/fallen version                      # sürüm bilgisi
```

`--stats`; lexer/parser/semantik/kodgen sürelerini, sınıf boyut/hizalamalarını,
fonksiyon çerçevelerini ve toplam statik stack rezervini yazdırır. `check`
komutu kod üretmez; `--stats` ile semantik analiz özetini de yazdırır.

### Test durumu

`tests/run.sh` şunları doğrular:

- `hello`: temel printf + aritmetik.
- `oop`: class, constructor, metot, private alan, dizi + döngü.
- `bounds`: çalışma zamanı dizi sınır ihlalinin yakalanması (çıkış kodu 1).
- `macros`: object-like ve function-like `#define`.
- `abi`: negatif int dönüş değeri ve char dönüşünün doğru işaret genişletmesi.
- `globals`, `types`, `multidecl`, `loop`: global başlatıcılar, unsigned/işaretli
  dönüşümler, çoklu bildirim ve döngü kontrolü.
- `switch`, `switch_default`: case/default, default ortada ve fallthrough.
- `multidim`, `arrayparam`: çok boyutlu diziler ve dizi parametre decay'i.
- `proto`, `suffix`: fonksiyon prototipleri ve integer sonekleri.
- `shell`: gerçek shell programının çalışması + hatalı komutta `perror` ve
  çocuk sürecin güvenli şekilde sonlanması.
- Negatif testler: `break/continue`, dizi boyutu, global tip, sınıf kopyası,
  dönüş tipi, argüman sayısı, tanımsız/çift fonksiyon, private constructor,
  `this` ataması, pointer üzerinde `.` ve çift `case` reddedilir.

---

## 3. Mimari / dosya haritası

```
Cargo.toml
src/
  main.rs        CLI, sürücü, temp dosyalar, as/ld çağrıları, stats
  lexer.rs       Token üretimi, yorumlar, sayı/string/char literalleri
  preprocess.rs  #include satırlarını temizler, #define makrolarını genişletir
  ast.rs         AST tipleri (Expr, Stmt, FuncDecl, ClassDecl, VarDecl...)
  parser.rs      C-benzeri recursive descent parser + precedence climbing
  types.rs       Type enum, boyut/hizalama, pointer/array yardımcıları
  sema.rs        Sınıf yerleşimi, kapsamlar, tip atama, slot/bellek analizi
  codegen.rs     SysV x86-64 assembly üretimi, runtime yardımcıları
examples/
  shell.flen     ana referans program
tests/
  *.flen         regresyon programları
  run.sh         uçtan uca test betiği
```

### Derleme akışı (`src/main.rs`)

1. Kaynak oku.
2. `preprocess::preprocess` → `Vec<Token>` (includes temizlenir, makrolar açılır).
3. `Parser::parse_program` → `Program` AST.
4. `Sema::analyze` → tipleri çözer, sınıf layout'larını ve stack çerçevelerini
   hesaplar, AST düğümlerini `ty`, `place`, `call_label` ile işaretler.
5. `Codegen::generate` → assembly string.
6. `as --64` → `.o`.
7. `link_direct` → doğrudan `ld` (crt1/crti/crtn + libc + dynamic linker).
   Bulunamazsa `cc -no-pie` fallback.
8. `run` komutunda geçici çıktı çalıştırılır ve silinir.

---

## 4. Dil kapsamı (v0.3)

### Desteklenen

- Tipler: `void`, `char`/`signed char`/`unsigned char`, `short`, `int`,
  `unsigned`, `long`, `unsigned long`, pointer, çok boyutlu sabit boyutlu dizi.
- Hazır typedef'ler: `pid_t`, `size_t`, `ssize_t`, `int8_t`...`uint64_t`,
  `bool`/`_Bool`, `FILE` (opaque).
- Fonksiyonlar: tanımlar, prototipler, parametreler, dönüş değeri, özyineleme.
  Dizi parametreleri otomatik olarak pointer'a decay eder.
- Kontrol: `if/else`, `while`, `do/while`, `for`, `switch/case/default`,
  `break`, `continue`, `return`.
- Operatörler: atama ve bileşik atama, `++/--`, aritmetik, karşılaştırma,
  mantıksal (`&&`, `||`, `!`), bitwise, shift, ternary, adres (`&`), dereference (`*`).
- `sizeof(type)` ve `sizeof(expr)` (derleme zamanında sabit).
- String ve char literalleri, escape dizileri, UTF-8 string baytları,
  onaltılık/ikilik/sekizlik integer literaller ve `u`/`U`/`l`/`L` sonekleri.
- Yerel `char dizi[N] = "metin"` başlatması.
- Global değişkenler: sabit int veya string başlatıcı; aksi halde `.bss`.
- `#include` satırları yok sayılır (libc imzaları derleyiciye gömülü).
- `#define NAME ...` object-like; `#define F(x) ...` function-like makrolar.
- OOP:
  - `class Name { private: ... protected: ... public: ... };`
  - Alanlar (dizi alanları dahil), metotlar, constructor.
  - `this`, `this->alan`, `nesne.alan`, `ptr->alan`, `nesne.metot()`, `ptr->metot()`.
  - Otomatik constructor çağrısı (`Shell my_shell;`).
  - Parametreli constructor (`Point p(3, 4);`).
  - Private/protected alan, metot ve constructor erişim denetimi.
  - `this` atanamaz; pointer üzerinde `.` ve değer üzerinde `->` reddedilir.
  - Sınıf boyutu ve hizalaması sema'da hesaplanır.
- Gömülü libc fonksiyonları: `printf`, `fprintf`, `perror`, `fflush`, `strtok`,
  `strcmp`, `strcspn`, `memset`, `fgets`, `chdir`, `fork`, `waitpid`, `execvp`,
  `strlen`, `malloc`, `calloc`, `free`, `memcpy`, `memcmp`, `atoi`, dosya/süreç
  fonksiyonları ve daha fazlası.
- Gömülü global'ler: `stdin`, `stdout`, `stderr`.
- Makro/eşdeğerleri: `NULL`, `WUNTRACED`, `WIFEXITED(...)`, `WIFSIGNALED(...)`.

### Desteklenmeyen / eksik

- Gerçek `float`/`double` yok; şu an `int` olarak eşleniyor.
- `struct`, `union`, `enum`, `typedef`, `goto` yok (parser'da kısmi
  `struct/enum/union` kalıntısı var, güvenilir değil).
- Kalıtım, sanal metot, destructor, operatör aşırı yükleme yok.
- Fonksiyon pointer'ları yok.
- `new`/`delete` ve heap sahiplik analizi yok. Model tamamen statik stack.
- 6'dan fazla argümanlı çağrılar yok (SysV stack argümanları yok).
- Aggregate (class/dizi) dönüş değeri ve değerle aggregate parametre geçişi yok.
- Global class nesneleri için otomatik constructor yok.
- Sınıf dizilerinde eleman constructor'ları otomatik çağrılmıyor.
- Brace initializer (`int a[3] = {1,2,3};`) yok.
- Header dosyaları gerçekten ayrıştırılmıyor; `#include` sadece atılıyor.
- Tam Rust borrow-checker seviyesinde alias/ömür analizi yok.
- Pointer'a decay olmuş dizilerde sınır bilgisi kaybolur; null/sıfıra bölme/alias
  analizi yok.
- Tip dönüşümleri hâlâ C kadar esnek; pointer↔integer dönüşümlerinde tam katılık
  ve uyarılar yok.
- `#if/#ifdef`, `#undef`, stringification (`#`), token paste (`##`) ve çok satırlı
  makrolar yok.

---

## 5. Önemli tasarım kararları

### AST anotasyonları
Sema, AST'yi mutasyona uğratarak her `Expr`'e şunları yazar:

- `ty: Option<Type>` → ifadenin tipi.
- `place: Option<Place>` → `Local(rbp ofseti)`, `Global(ad)`, `ExternVar(ad)`.
- `call_label: Option<String>` → çözülmüş fonksiyon/metot sembolü.

`VarDecl.offset` ve `FuncDecl.frame_size` / `FuncDecl.this_offset` de sema
tarafından doldurulur. Codegen yeniden kapsam çözmez; bu anotasyonları kullanır.

### Sınıf yerleşimi (`sema::layout_class`)
- Alanlar sırayla hizalanır, `FieldInfo.offset` hesaplanır.
- Sınıf hizalaması alanların maksimum hizalaması; boyut hizalamaya yuvarlanır.
- Sınıf kendisini değer olarak içerirse hata.
- Metot sembolleri: `Sinif_metot`, constructor: `Sinif_ctor`.

### Yerel değişken / çerçeve hesabı
- Slotlar `rbp`'den negatif ofsetlerle ayrılır.
- Kapsam ömrü yeniden kullanımı (slot reuse) yoktur; tüm yereller toplanır.
  Bu bilinçli olarak "güvenli ve statik maksimum" yaklaşımıdır.
- `frame_size = align_up(kullanılan, 16)`.
- `--stats` her fonksiyon için çerçeve, yerel ve aggregate baytlarını yazar.

### SysV x86-64 kod üretimi
- Parametre register'ları: `rdi, rsi, rdx, rcx, r8, r9`. Metotta ilk argüman `this`.
- Çağrıda argümanlar soldan sağa değerlendirilip stack'e push edilir, sonra
  sondan başa register'lara pop edilir.
- Çağrı öncesi stack hizalaması gerekirse `sub $8, %rsp` ile düzeltilir.
- Varargs için `xor %eax, %eax` (0 vektör register).
- Çağrı dönüşünde `ret_ty` boyutuna göre işaret/zero genişletme yapılır
  (`movslq %eax, %rax` vb.). **Bu kritik bir ABI detayıdır.**
- İkili operasyonlarda konvansiyon: `a` → `%rax`, `b` → `%rcx`.
  - `sub %rcx, %rax`, `cmp %rcx, %rax`, `idiv %rcx`.
  - `shl %cl, %rax`.
- Pointer aritmetiğinde eleman boyutuyla ölçekleme yapılır.
- Bounds check: taban tipi `Array(_, n)` ise indeks `0 <= i < n` kontrol edilir;
  ihlalde `__fallen_bounds_fail` çağrılır. Sabit indeksler sema'da derleme hatası.
- Başlatılmamış yereller `rep stosb` veya doğrudan `movq $0` ile sıfırlanır.

### Fork/exec güvenliği (`codegen::child_branch_of`)
- `fork()` sonucu bir değişkene atanırsa bu değişken `fork_vars` kümesine girer.
- `if (pid == 0)` çocuk dalı, `if (pid != 0)` ise else dalı çocuk kabul edilir.
- Çocuk dalı sonunda kontrol akışı devam ediyorsa otomatik `_exit(127)` eklenir.
- Bu, shell örneğindeki "başarısız exec sonrası çocuk shell'e dönme" hatasını
  derleyici seviyesinde kapatır.
- `falls_through` basit kontrol akışı analizi yapar.

### Önişlemci (`preprocess.rs`)
- Kaynak satır satır taranır; `#define` tanımları toplanır, diğer `#...` satırları
  boş satıra dönüştürülür (satır numaraları korunur).
- Makro gövdeleri tokenize edilir, token seviyesinde genişletme yapılır.
- Fonksiyon-benzeri makrolarda argümanlar virgül/parantez dengesiyle ayrılır.
- 64 seviye genişletme sınırı vardır (özyineleme koruması).

---

## 6. Bilinen tuzaklar / dikkat edilecekler

1. **Dönüş değeri işaret genişletmesi**: `int` dönen fonksiyonların üst 32 biti
   tanımsızdır. `gen_call` içinde `ret_ty`'ye göre normalize edilmezse
   `execvp(...) == -1` gibi karşılaştırmalar yanlış çalışır. Bu daha önce yaşandı
   ve düzeltildi; yeni çağrı yolları eklerken koru.
2. **Operand sırası**: `cmp %rcx, %rax` AT&T'de `rax - rcx` demektir. Sol taraf
   `rax`, sağ taraf `rcx` konvansiyonunu bozma.
3. **Stack hizalaması**: Her `push`/`pop` çifti dengeli olmalı. `align_call` /
   `unalign_call(padded)` çiftini birlikte kullan.
4. **Array decay**: `Array(T, n)` ifadeleri değer olarak kullanıldığında adres
   döndürür; `load_at` aggregate tipleri yüklemez.
5. **`sizeof`**: `SizeofExpr` iç ifadeyi değerlendirmez; sadece tipinin boyutunu
   kullanır.
6. **Metot çağrısı**: `Call` düğümünün `call_label` alanı kullanılır (callee'nin
   değil). Metot çağrısında `this` ilk argüman olarak hazırlanır.
7. **`as` sözdizimi**: `.globl` ve sembol adı aynı satırda olmalı.
8. **Parser belirsizliği**: `looks_like_decl` sınıf/typedef adlarını kullanır.
   Yeni tip sözdizimi eklerken burayı güncelle.
9. **Bounds check**: Şu an yalnızca gerçek `Array` tabanlarında var. Pointer'a
   decay olmuş dizilerde (`char *p = buf; p[10000]`) sınır bilgisi kaybolur.
   Gelecekte "fat pointer" veya kaynak takibi gerekir.
10. **Global başlatıcılar**: Sadece sabit int, `-int` ve string desteklenir.
    `const_init` genişletilirse codegen'deki `.data` üretimini güncelle.

---

## 7. Geliştirmeye devam için önerilen sıralama

Öncelik sırasına göre:

1. **Tip sistemi sertleştirme**
   - Gerçek `float`/`double` + SSE register ABI.
   - Dönüş tipi uyumu, implicit conversion kuralları ve uyarılar.
   - `size_t`/unsigned karşılaştırmalarının tam C semantiği.

2. **C kapsamını genişletme**
   - `struct`, `union`, `enum`, `typedef`, `goto`.
   - Brace initializer'lar (`{1,2,3}`, nested).
   - Fonksiyon pointer'ları.
   - 6'dan fazla argüman (SysV stack argümanları).

3. **OOP derinleştirme**
   - Kalıtım, alan/metot override, sanal dispatch.
   - Destructor ve kapsam sonu otomatik yıkım.
   - Sınıf dizilerinde otomatik constructor.
   - Global class nesnelerinde constructor.

4. **Bellek modeli**
   - `new`/`delete` ve heap sahiplik/ömür analizi.
   - Pointer kaynak takibi ile decay sonrası bounds koruması.
   - Slot ömrü analizi ile daha küçük çerçeveler (şu an muhafazakâr toplam).
   - Kullanılmayan/ölü değişken eleme.

5. **Önişlemci ve başlıklar**
   - `#include` için gerçek başlık ayrıştırma veya gömülü imza kataloğu.
   - `#if/#ifdef`, `#undef`, stringification (`#`), token paste (`##`).
   - Çok satırlı `\` devamları.

6. **Tanılama ve araçlar**
   - Satır/kolon bazlı hata mesajları.
   - `--emit-ast`, `--emit-ir` hata ayıklama çıktıları.
   - Derleme süresi/bellek raporunu JSON olarak verme.

7. **Test altyapısı**
   - Her dil özelliği için ayrı `.flen` regresyon testi.
   - Negatif testler (derlenmemesi gereken programlar).
   - `as`/`ld` olmadan sadece assembly karşılaştırmalı golden testler.

---

## 8. Hızlı başlangıç kontrol listesi

Yeni oturumda:

```bash
cd /home/berat/fallen
git status
cargo build --release
./tests/run.sh
target/release/fallen build examples/shell.flen -o /tmp/fallensh --stats
printf 'echo test\nexit\n' | /tmp/fallensh
```

Hepsi geçiyorsa mevcut durum sağlamdır; doğrudan yukarıdaki yol haritasından
bir maddeye geçebilirsin.

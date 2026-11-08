# Fallen

C benzeri sözdizimine sahip, dizi sınır kontrolü hedefleyen ve doğrudan x86-64 makine koduna derlenen deneysel bir programlama dili ve derleyicisi. Sürüm: **0.3.0**.

Fallen, `.flen` uzantılı kaynak dosyalarını GNU assembly'ye, oradan da `as` ve `ld` ile doğrudan çalıştırılabilir bir dosyaya dönüştürür. Araya C derleyicisi girmez; `cc` yalnızca `ld` bulunamadığında yedek bağlayıcı olarak kullanılır.

## Özellikler

- **C benzeri sözdizimi**: değişkenler, pointer'lar, çok boyutlu diziler, fonksiyonlar/prototipler, kontrol akışı, `switch/case`, `#include` ve `#define`.
- **Bellek güvenliği**: doğrudan dizi erişimlerinde çalışma zamanı sınır kontrolü, otomatik sıfır başlatma, private/protected erişim denetimi ve fork/exec güvenliği.
- **OOP**: `class`, `private`/`protected`/`public`, alanlar, metotlar, constructor, `this`, `.` ve `->`, otomatik constructor çağrısı.
- **Statik bellek analizi**: sınıf boyutları, alan ofsetleri, her fonksiyonun stack çerçevesi ve toplam statik stack rezervi derleme anında hesaplanır.
- **Doğrudan makine kodu**: `.flen` → x86-64 GNU assembly → `as` → `ld`.
- **Hızlı derleme**: örnek shell programında derleyici aşamaları yaklaşık 0.5 ms, `as + ld` dahil toplam yaklaşık 20 ms.
- **Kontroller**: dönüş tipi, fonksiyon argüman sayısı/tipi, atama tipi, tanımsız fonksiyon ve çift tanımlı semboller için derleme zamanı denetimleri.

## Kurulum

Rust araç zinciri ve `binutils` (`as`, `ld`) gerekir.

```bash
git clone https://github.com/7yokki/fallen.git
cd fallen
cargo build --release
```

Derleyici `target/release/fallen` altında oluşur.

## Kullanım

```bash
# Derle
target/release/fallen build dosya.flen -o cikti

# Derle ve çalıştır
target/release/fallen run dosya.flen

# Yalnızca assembly üret
target/release/fallen build dosya.flen -S

# Derleme istatistiklerini göster (süreler, sınıf yerleşimi, stack çerçeveleri)
target/release/fallen build dosya.flen -o cikti --stats

# Üretilen assembly dosyasını koru
target/release/fallen build dosya.flen -o cikti --keep-asm

# Kod üretmeden yalnızca semantik kontrol yap
target/release/fallen check dosya.flen

# Sürümü göster
target/release/fallen version
```

### Örnek

```c
#include <stdio.h>

class Point {
private:
    int x;
    int y;
public:
    Point(int a, int b) { this->x = a; this->y = b; }
    int sum() { return this->x + this->y; }
};

int main() {
    Point p(3, 4);
    printf("p=%d\n", p.sum());
    return 0;
}
```

```bash
target/release/fallen run ornek.flen
```

Referans program olarak minimalist, sınıf tabanlı bir shell olan [`examples/shell.flen`](examples/shell.flen) bulunur.

## Desteklenen dil kapsamı

- **Tipler**: `void`, `char`, `short`, `int`, `unsigned`, `long`, `unsigned long`, pointer, çok boyutlu sabit boyutlu diziler ve dizi parametrelerinde otomatik pointer decay.
- **Hazır typedef'ler**: `pid_t`, `size_t`, `ssize_t`, `int8_t`...`uint64_t`, `bool`/`_Bool`, `FILE`.
- **Fonksiyonlar**: tanımlar, prototipler, parametreler, dönüş değeri, özyineleme.
- **Kontrol akışı**: `if/else`, `while`, `do/while`, `for`, `switch/case/default`, `break`, `continue`, `return`.
- **Operatörler**: atama ve bileşik atama, `++/--`, aritmetik, karşılaştırma, mantıksal, bitwise, shift, ternary, adres ve dereference.
- **Diğer**: `sizeof`, string/char literalleri, onaltılık/ikilik/sekizlik sayılar ve `u`/`l` sonekleri, yerel dizi başlatma, global değişkenler, object-like ve function-like makrolar.
- **Gömülü libc fonksiyonları**: `printf`, `fprintf`, `perror`, `strtok`, `strcmp`, `strcspn`, `memset`, `fgets`, `fork`, `waitpid`, `execvp`, `strlen`, `malloc`, `calloc`, `free`, `memcpy`, `memcmp`, `atoi` ve daha fazlası.
- **Gömülü global'ler**: `stdin`, `stdout`, `stderr`.

### Henüz desteklenmeyenler

- Gerçek `float`/`double` (şu an `int` olarak eşleniyor).
- `struct`, `union`, `enum`, `typedef`, `goto`.
- Kalıtım, sanal metotlar, destructor, operatör aşırı yükleme.
- Fonksiyon pointer'ları.
- `new`/`delete` ve heap sahiplik analizi (model tamamen statik stack).
- 6'dan fazla argümanlı çağrılar.
- Aggregate (dizi/sınıf) dönüş değerleri ve değerle aggregate parametre geçişi.
- Brace initializer (`int a[3] = {1,2,3};`).
- Gerçek header ayrıştırma (`#include` satırları atılır).
- Pointer'a decay olmuş dizilerde sınır bilgisinin korunması; null/sıfıra bölme/alias/ömür analizi.
- `#if/#ifdef`, `#undef`, `#`/`##` ve çok satırlı makrolar.

## Mimari

```
src/
  main.rs        CLI, sürücü, geçici dosyalar, as/ld çağrıları, istatistikler
  lexer.rs       Token üretimi, yorumlar, sayı/string/char literalleri
  preprocess.rs  #include temizliği, #define makro genişletme
  ast.rs         AST tipleri
  parser.rs      C-benzeri recursive descent parser
  types.rs       Type enum, boyut/hizalama yardımcıları
  sema.rs        Sınıf yerleşimi, kapsamlar, tip atama, slot analizi
  codegen.rs     SysV x86-64 assembly üretimi, runtime yardımcıları
examples/
  shell.flen     Referans program
tests/
  *.flen         Regresyon testleri
  run.sh         Uçtan uca test betiği
```

### Derleme akışı

1. Kaynak dosya okunur.
2. Önişlemci `#include` satırlarını temizler ve makroları genişletir.
3. Parser programı AST'ye dönüştürür.
4. Sema tipleri çözer, sınıf yerleşimlerini ve stack çerçevelerini hesaplar.
5. Codegen SysV x86-64 assembly üretir.
6. `as --64` ile nesne dosyası oluşturulur.
7. `ld` ile doğrudan bağlanır; `ld` yoksa `cc` yedeği kullanılır.

## Testler

```bash
./tests/run.sh
```

Testler; temel `printf` ve aritmetiği, OOP'yi, dizi sınır kontrolünü, makroları, ABI dönüş değerlerini, `switch` (default ortada dahil), çok boyutlu dizileri, dizi parametre decay'ini, prototipleri, sayı soneklerini, tip/argüman/dönüş denetimlerini, erişim kontrollerini ve referans shell programını doğrular. Negatif testler geçersiz programların derlenmediğini kontrol eder.

## Lisans

Bu proje [MIT Lisansı](LICENSE) altında lisanslanmıştır.

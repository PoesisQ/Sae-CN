# 原生公式 / Native mathematics

正文中的 $u_{\mathrm{rest}}$ 与 $I_C=C\frac{du}{dt}$ 随字号重排。Inline math $E=mc^2$ remains selectable.

$$
\tau\frac{du}{dt}=-(u-u_{\mathrm{rest}})+RI(t)
$$

$$
x=\frac{-b\pm\sqrt{b^2-4ac}}{2a}
$$

$$
\begin{aligned}
I(t)&=I_C+I_R\\
RI(t)&=RC\frac{du}{dt}+u-u_{\mathrm{rest}}
\end{aligned}
$$

## 矩阵与求和

```math
A=\begin{pmatrix}1&2\\3&4\end{pmatrix},\quad
S=\sum_{i=1}^{n}i^2,\quad Q=\int_0^1 x^2\,dx
```

### 代码、货币与降级

代码 `u_rest` 和 `$x^2$` 保留原文；价格 \$5 和 \$10。

```rust
let u_rest = -65;
// $not_math$ stays code.
```

暂不支持的公式 $\unknowncommand{x}$ 应保留源码。

[返回标题](#原生公式-/-native-mathematics)

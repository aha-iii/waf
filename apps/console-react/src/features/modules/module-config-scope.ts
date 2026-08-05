/** 防护模块配置作用域：与核心防护三层模型一致 */
export type ModuleConfigScope = "global" | "strategy" | "app";

export function moduleScopeHint(scope: ModuleConfigScope): string {
  switch (scope) {
    case "global":
      return "全局默认：未单独配置的应用与策略继承此模板。";
    case "strategy":
      return "策略快照：仅写入当前策略模板；应用到站点后才会生效，与全局默认、应用覆盖相互隔离。";
    case "app":
      return "本应用生效配置：与全局默认、策略库分离；绑定内置策略时为只读，可点策略卡片同步或创建本应用私有策略。";
  }
}

export function preAccessDetailLinkVisible(scope: ModuleConfigScope): boolean {
  return scope === "global";
}

export function preAccessCardScopeNote(
  _scope: ModuleConfigScope,
  kind: "pre_access" | "response",
): string {
  if (kind === "response") return "";
  return "";
}

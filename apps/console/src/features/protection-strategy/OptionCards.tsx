/** protection-strategy feature: Option Cards. */
type Option = { value: string; label: string; desc: string };

type Props = {
  name: string;
  options: readonly Option[];
  value: string;
  onChange: (value: string) => void;
};

/** Radio option cards aligned with legacy Console pl-option-group */
export function OptionCards({ name, options, value, onChange }: Props) {
  return (
    <div className="pl-option-group" role="radiogroup">
      {options.map((o) => (
        <label
          key={o.value}
          className={`pl-option${o.value === value ? " pl-option-active" : ""}`}
        >
          <input
            type="radio"
            name={name}
            value={o.value}
            checked={o.value === value}
            onChange={() => onChange(o.value)}
          />
          <span className="pl-option-label">{o.label}</span>
          <span className="pl-option-desc">{o.desc}</span>
        </label>
      ))}
    </div>
  );
}

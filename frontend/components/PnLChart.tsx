import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  Legend,
  ResponsiveContainer,
} from "recharts";

type Props = {
  strategies: Array<{
    id: string;
    title: string;
    risk_score: number;
    projected_return_7d: { best_case: string; expected: string; worst_case: string };
  }>;
};

function parseDollar(s: string): number {
  return parseFloat(s.replace("$", ""));
}

export default function PnLChart({ strategies }: Props) {
  const data = strategies.map((s) => ({
    name: s.title.slice(0, 12),
    worst_case: parseDollar(s.projected_return_7d.worst_case),
    expected: parseDollar(s.projected_return_7d.expected),
    best_case: parseDollar(s.projected_return_7d.best_case),
  }));

  return (
    <ResponsiveContainer width="100%" height={280}>
      <BarChart data={data} margin={{ top: 8, right: 16, left: 8, bottom: 8 }}>
        <CartesianGrid strokeDasharray="3 3" />
        <XAxis dataKey="name" />
        <YAxis tickFormatter={(v) => `$${v}`} />
        <Tooltip formatter={(value: number) => `$${value}`} />
        <Legend verticalAlign="bottom" />
        <Bar dataKey="worst_case" name="Worst Case" fill="#ef4444" />
        <Bar dataKey="expected" name="Expected" fill="#3b82f6" />
        <Bar dataKey="best_case" name="Best Case" fill="#22c55e" />
      </BarChart>
    </ResponsiveContainer>
  );
}

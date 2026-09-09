import { Placeholder } from "../components/ui";

export default function Spending() {
  return (
    <>
      <h1 className="mb-6 text-xl font-semibold">Spending</h1>
      <Placeholder
        title="Coming in milestone 3"
        note="Once a Plaid item is linked, this page shows the expandable category → merchant → transaction breakdown, monthly totals, and spend-over-time charts."
      />
    </>
  );
}

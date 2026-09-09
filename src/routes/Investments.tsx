import { Placeholder } from "../components/ui";

export default function Investments() {
  return (
    <>
      <h1 className="mb-6 text-xl font-semibold">Investments</h1>
      <Placeholder
        title="Coming in milestone 5"
        note="After connecting Robinhood and E*Trade through SnapTrade, this page shows combined and per-account holdings with allocation, cost basis, and portfolio value over time."
      />
    </>
  );
}

-- Returns one report only when the caller owns the containing project.
create or replace function get_tender_report(p_user_id uuid, p_report_id uuid)
returns jsonb as $$
declare
    v_report jsonb;
begin
    select to_jsonb(report) into v_report
    from (
        select
            report.report_id,
            report.job_id,
            report.document_id,
            report.result,
            report.contract_version,
            report.created_at
        from tender_analysis_reports report
        join tender_documents document on document.document_id = report.document_id
        join tender_projects project on project.project_id = document.project_id
        where report.report_id = p_report_id
          and project.user_id = p_user_id
    ) report;

    return v_report;
end;
$$ language plpgsql;
